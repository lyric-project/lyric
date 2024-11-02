use chrono::Local;
use std::fmt::Debug;
use std::mem;
use std::sync::Arc;
use tokio::sync::{mpsc, oneshot};
use tracing;
use uuid::Uuid;

use crate::config::Config;
use crate::env::{EnvironmentConfigMessage, EnvironmentID, WorkerEnvManager};
use crate::message::{
    LangWorkerMessage, NotifyMessage, PendingTask, ResultSender, RpcMessage, TaskStateResult,
    TriggerScheduleEvent,
};
use crate::task::{TaskDescription, TaskID};
use crate::worker::WorkerID;
use lyric_rpc::task::{
    ExecutionMode, Language, RegisterWorkerRequest, TaskStateInfo, TaskStateRequest,
};
use lyric_utils::prelude::*;

#[derive(Debug)]
pub(crate) struct CoreLyric {
    #[allow(dead_code)]
    pub(crate) tx_api: mpsc::UnboundedSender<RpcMessage>,
    pub(crate) rx_api: mpsc::UnboundedReceiver<RpcMessage>,
    pub(crate) tx_notify: mpsc::UnboundedSender<NotifyMessage>,
    pub(crate) rx_notify: mpsc::UnboundedReceiver<NotifyMessage>,
    /// Sender message to language worker
    pub(crate) tx_lang_worker: Option<mpsc::UnboundedSender<LangWorkerMessage>>,
    pub(crate) worker_manager: WorkerEnvManager,
    pub(crate) pending_tasks: Vec<PendingTask>,
    pub(crate) config: Arc<Config>,
}

impl CoreLyric {
    pub(crate) async fn main(mut self, rx_shutdown: oneshot::Receiver<()>) -> Result<(), Error> {
        tracing::info!("CoreLyric started");
        let res = self.runtime_loop(rx_shutdown).await;
        // TODO: Cleanup all resources
        tracing::debug!("Begin to stop CoreLyric");
        self.stop().await;
        tracing::debug!("CoreLyric stopped");
        match res {
            Ok(_) => {
                tracing::info!("CoreLyric stopped");
                Ok(())
            }
            Err(e) => {
                tracing::debug!("CoreLyric stopped with error: {:?}", e);
                Err(e)
            }
        }
    }

    async fn runtime_loop(&mut self, mut rx_shutdown: oneshot::Receiver<()>) -> Result<(), Error> {
        let mut cleanup_interval = tokio::time::interval(std::time::Duration::from_secs(60));
        let mut trigger_schedule_interval =
            tokio::time::interval(std::time::Duration::from_secs(10));
        loop {
            tokio::select! {
                // Check shutdown in each loop first so that a message flood in `tx_api` won't block shutting down.
                // `select!` without `biased` provides a random fairness.
                // We want to check shutdown prior to other channels.
                // See: https://docs.rs/tokio/latest/tokio/macro.select.html#fairness
                biased;

                _ = &mut rx_shutdown => {
                    return Err(Error::CoreStopped("recv from rx_shutdown".to_string()));
                }

                notify_res = self.rx_notify.recv() => {
                    match notify_res {
                        Some(notify) => self.handle_notify(notify).await?,
                        None => {
                            return Err(Error::CoreStopped("rx_notify senders are dropped".to_string()));
                        }
                    }
                }

                msg_res = self.rx_api.recv() => {
                    match msg_res {
                        Some(msg) => self.handle_api_msg(msg).await?,
                        None => {
                            return Err(Error::CoreStopped("rx_api senders are dropped".to_string()));
                        }
                    }
                }

                _ = cleanup_interval.tick() => {
                     self.worker_manager.cleanup_completed_tasks().await;
                }
                _ = trigger_schedule_interval.tick() => {
                    let _ = self.trigger_schedule(TriggerScheduleEvent::TimeInterval);
                }
            }
        }
    }

    #[tracing::instrument(level = "debug", skip_all, fields(msg = ?msg.trace_info()))]
    pub(crate) async fn handle_notify(&mut self, msg: NotifyMessage) -> Result<(), Error> {
        match msg {
            NotifyMessage::TriggerSchedule { event, timestamp } => {
                tracing::debug!(
                    "Trigger schedule event: {:?}, timestamp: {}",
                    event,
                    timestamp
                );
                self.schedule().await;
            }
            NotifyMessage::CreateWorkerResult { worker_id, process } => {
                self.worker_manager
                    .handle_create_worker_result(worker_id, process)
                    .await;
            }

            NotifyMessage::RetryScheduleTask { pending_task } => {
                if pending_task.retry_times < 3 {
                    let rpc = pending_task.task.clone();
                    self.pending_tasks.push(pending_task);
                    self.trigger_schedule(TriggerScheduleEvent::SubmitTask(rpc))
                        .unwrap();
                } else {
                    tracing::info!(
                        "Task {:?} retry times exceed limit, discard",
                        pending_task.task.task_id
                    );
                    let _ = pending_task.tx.send(Err(Error::ExecutionError(
                        "Task retry times exceed limit".to_string(),
                    )));
                }
            }
        }
        Ok(())
    }

    #[tracing::instrument(level = "debug", skip_all, fields(msg = ?msg.trace_info()))]
    pub(crate) async fn handle_api_msg(&mut self, msg: RpcMessage) -> Result<(), Error> {
        match msg {
            RpcMessage::SubmitTask { rpc, tx, env } => {
                if self.config.is_driver {
                    tracing::info!("Receive SubmitTask message, {:?}, current is driver", rpc);
                    match self.handle_submit_task_remote(rpc, tx, env).await {
                        Ok(_) => {
                            tracing::info!("Task submitted");
                        }
                        Err(e) => {
                            tracing::warn!("Task submit failed: {:?}", e);
                        }
                    }
                } else {
                    let worker_id = self.config.node_id.as_ref().unwrap().to_string();
                    tracing::info!("Receive SubmitTask message, {:?}, current is worker", rpc);
                    self.handle_submit_on_worker(rpc, tx, worker_id).await;
                }
            }
            RpcMessage::StopTask { task_id, tx } => {
                if self.config.is_driver {
                    tracing::info!("Receive StopTask message, {:?}", task_id);
                    self.worker_manager.stop_task(task_id, tx).await;
                } else {
                    tracing::info!("Receive StopTask message, {:?}", task_id);
                    self.handle_stop_task_on_worker(task_id, tx).await;
                }
            }
            RpcMessage::TaskStateChange(req) => {
                tracing::info!("Receive TaskStateChange message, {:?}", req);
                self.handle_task_completed(req).await;
            }
            RpcMessage::RegisterWorker(req) => {
                tracing::info!("Receive RegisterWorker message, {:?}", req);

                match self.handle_register_worker(req).await {
                    Ok(_) => {
                        tracing::info!("Worker registered");
                    }
                    Err(e) => {
                        tracing::error!("Worker register failed: {:?}", e);
                    }
                }
            }
            _ => {
                // ignore
            }
        }
        Ok(())
    }

    async fn handle_submit_on_worker(
        &mut self,
        task_rpc: TaskDescription,
        tx: ResultSender<TaskStateResult, Error>,
        worker_id: String,
    ) {
        match task_rpc.language {
            Some(Language::Shell) => {
                tracing::info!("Current language is shell, send to shell worker");
                // self.handle_submit_task_local(rpc, tx, worker_id).await;
                todo!()
            }
            _ => {
                tracing::info!("Current language is not 2, send to language worker");
                match self.tx_lang_worker.as_ref() {
                    Some(tx_lang_worker) => match task_rpc.exec_mode {
                        ExecutionMode::Local => {
                            let msg = LangWorkerMessage::SubmitTask {
                                rpc: task_rpc,
                                tx,
                                worker_id,
                            };
                            let _ = tx_lang_worker.send(msg);
                        }
                        _ => {
                            let msg = LangWorkerMessage::SubmitLaunchComponent {
                                rpc: task_rpc,
                                tx,
                                worker_id,
                            };
                            let _ = tx_lang_worker.send(msg);
                        }
                    },
                    None => {
                        tracing::error!("Language worker tx is not ready");
                    }
                }
            }
        }
    }

    async fn handle_stop_task_on_worker(&mut self, task_id: TaskID, tx: ResultSender<(), Error>) {
        match self.tx_lang_worker.as_ref() {
            Some(tx_lang_worker) => {
                // TODO: Just support stop task for component task(WASM task) now
                let msg = LangWorkerMessage::StopComponentTask {
                    task_id,
                    tx,
                    worker_id: self.config.node_id.as_ref().unwrap().to_string(),
                };
                let _ = tx_lang_worker.send(msg);
            }
            None => {
                tracing::error!("Language worker tx is not ready");
                let _ = tx.send(Err(Error::InternalError(
                    "Language worker tx is not ready".to_string(),
                )));
            }
        }
    }

    async fn handle_submit_task_remote(
        &mut self,
        task_info: TaskDescription,
        tx: ResultSender<TaskStateResult, Error>,
        env: Option<EnvironmentConfigMessage>,
    ) -> Result<(), Error> {
        let language = task_info
            .language
            .ok_or(Error::InternalError("Task language is not set".to_string()))?;

        let env = env.unwrap_or(EnvironmentConfigMessage::default());
        let env_id = env.id();
        let worker_id = WorkerID::new(env_id.as_str(), Uuid::new_v4().to_string().as_str());
        let task_info_clone = task_info.clone();
        let pending_task = PendingTask {
            task: task_info.clone(),
            tx,
            env: env.clone(),
            worker_id: worker_id.clone(),
            retry_times: 0,
        };
        if let Some(_) = self.worker_manager.find_any_idle_worker(env_id.as_str()) {
            // Append to the task queue
            self.pending_tasks.push(pending_task);
        } else if self.worker_manager.can_launch_worker() {
            // Try to start a new worker
            if let Err(e) = self
                .worker_manager
                .launch_worker(worker_id, language, env)
                .await
            {
                tracing::error!("Failed to start new worker: {:?}", e);
                // If we can't start a new worker, add the task to the waiting queue
                self.pending_tasks.push(pending_task);
            } else {
                // If we successfully started a new worker, it will register itself
                // and then we can assign the task in the next event loop iteration
                self.pending_tasks.push(pending_task);
            }
        } else {
            // If we've reached the maximum number of workers, add the task to the waiting queue
            self.pending_tasks.push(pending_task);
        }
        self.trigger_schedule(TriggerScheduleEvent::SubmitTask(task_info_clone))?;
        Ok(())
    }

    async fn assign_task_to_worker(&mut self, pending_task: PendingTask) {
        self.worker_manager
            .assign_task_to_worker(pending_task, self.tx_api.clone())
            .await;
    }

    async fn handle_register_worker(&mut self, req: RegisterWorkerRequest) -> Result<(), Error> {
        let req_copy = req.clone();
        match self.worker_manager.handle_register_worker(req).await {
            Ok(()) => self.trigger_schedule(TriggerScheduleEvent::RegisterWorker(req_copy)),
            Err(e) => Err(e),
        }
    }

    fn trigger_schedule(&self, event: TriggerScheduleEvent) -> Result<(), Error> {
        if !self.config.is_driver {
            return Ok(());
        }
        let _ = self.tx_notify.send(NotifyMessage::TriggerSchedule {
            event,
            timestamp: Local::now().timestamp_millis(),
        });
        Ok(())
    }

    async fn schedule(&mut self) {
        let num_pending_tasks = self.pending_tasks.len();
        let num_idle_workers = self.worker_manager.idle_worker_count();
        let total_workers = self.worker_manager.worker_handles.len();
        let num_pending_workers = self.worker_manager.pending_workers.len();
        tracing::info!(
            "Scheduling: {} pending tasks, {} idle workers, {} pending workers, {} total workers",
            num_pending_tasks,
            num_idle_workers,
            num_pending_workers,
            total_workers
        );
        // Use mem::take to avoid borrow conflict
        let tasks_to_schedule = mem::take(&mut self.pending_tasks);
        let mut new_pending_tasks = Vec::with_capacity(tasks_to_schedule.len());
        for mut pending_task in tasks_to_schedule {
            if let Some(worker_id) = self
                .worker_manager
                .find_any_idle_worker(pending_task.env.id().as_str())
            {
                pending_task.worker_id = worker_id;
                self.assign_task_to_worker(pending_task).await;
            } else {
                new_pending_tasks.push(pending_task);
            }
        }
        // Assign the unassigned tasks back to pending_tasks
        self.pending_tasks = new_pending_tasks;
    }

    async fn handle_task_completed(&mut self, req: TaskStateRequest) {
        let req_copy = req.clone();
        self.worker_manager.handle_task_completed(req).await;
        self.trigger_schedule(TriggerScheduleEvent::TaskStateChange(req_copy))
            .unwrap();
    }

    async fn stop(&mut self) {
        // Stop language workers
        drop(self.tx_lang_worker.take());
        self.worker_manager.stop().await;
    }
}
