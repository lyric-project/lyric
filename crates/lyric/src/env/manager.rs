use crate::config::WorkerStartCommand;
use crate::env::config::EnvironmentBuilder;
use crate::env::docker::DockerEnvironmentBuilder;
use crate::env::env::{ChildProcess, ExecutionEnvironment};
use crate::env::local::LocalEnvironmentBuilder;
use crate::env::EnvironmentConfigMessage;
use crate::message::{NotifyMessage, PendingTask, RpcMessage, TaskStateResult};
use crate::rpc::RPC_PROTOCOL_VERSION;
use crate::task::TaskID;
use crate::worker::{connect_to_worker, EnvWorkerInfo, WorkerID, WorkerStatus};
use crate::{Config, ResultSender, TokioRuntime};
use chrono::{Duration, Local};
use lyric_rpc::task::{
    Language, RegisterWorkerRequest, TaskStateRequest, TaskStopRequest, TaskSubmitRequest,
};
use lyric_utils::err::TaskError;
use lyric_utils::net_utils::listen_available_port;
use lyric_utils::prelude::Error;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::sync::Mutex as StdMutex;
use tokio::sync::{mpsc, oneshot};
use tokio_stream::wrappers::UnboundedReceiverStream;
use tokio_stream::StreamExt;

#[derive(Debug)]
struct TaskHandle {
    task_id: TaskID,
    worker_id: WorkerID,
    handle: tokio::task::JoinHandle<()>,
}

#[derive(Debug)]
pub(crate) struct WorkerEnvManager {
    pub(crate) tx_core_notify: mpsc::UnboundedSender<NotifyMessage>,
    /// The worker IDs that are currently pending.
    pub(crate) pending_workers: HashMap<WorkerID, i64>,
    /// The worker handles.
    pub(crate) worker_handles: HashMap<WorkerID, Box<dyn ChildProcess>>,
    pub(crate) task_handles: HashMap<TaskID, TaskHandle>,
    task_id_to_worker_id: HashMap<TaskID, WorkerID>,
    /// The worker information for each registered worker.
    pub(crate) registered_workers: HashMap<WorkerID, EnvWorkerInfo>,
    pub(crate) assigned_ports: Arc<StdMutex<HashSet<u16>>>,
    pub(crate) config: Arc<Config>,
    launch_timeout: Duration,
    runtime: TokioRuntime,
}

impl WorkerEnvManager {
    pub fn new(
        config: Arc<Config>,
        tx_core_notify: mpsc::UnboundedSender<NotifyMessage>,
        runtime: TokioRuntime,
    ) -> Self {
        Self {
            tx_core_notify,
            pending_workers: HashMap::new(),
            worker_handles: HashMap::new(),
            task_handles: HashMap::new(),
            task_id_to_worker_id: HashMap::new(),
            registered_workers: HashMap::new(),
            assigned_ports: Arc::new(StdMutex::new(HashSet::new())),
            config,
            launch_timeout: Duration::seconds(30),
            runtime,
        }
    }

    /// Check if a worker can be launched.
    ///
    /// A worker can be launched if the number of registered workers and pending workers is less
    /// than the maximum number of workers.
    pub fn can_launch_worker(&self) -> bool {
        self.registered_workers.len() + self.pending_workers.len()
            < self.config.maximum_workers as usize
    }

    /// Get the number of idle workers.
    pub fn idle_worker_count(&self) -> usize {
        self.registered_workers
            .iter()
            .filter(|(_, info)| matches!(info.status, WorkerStatus::Idle))
            .count()
    }

    pub fn worker_count(&self) -> usize {
        self.registered_workers.len()
    }

    /// Find idle workers for a given environment.
    pub fn find_all_idle_workers<'a>(
        &'a self,
        env_id: &'a str,
    ) -> impl Iterator<Item = WorkerID> + 'a {
        self.registered_workers
            .iter()
            .filter(move |(id, info)| {
                id.env_id == env_id && matches!(info.status, WorkerStatus::Idle)
            })
            .map(|(id, _)| id.clone())
    }

    /// Find any idle worker for a given environment.
    pub fn find_any_idle_worker(&self, env_id: &str) -> Option<WorkerID> {
        self.registered_workers
            .iter()
            .find(|&(id, info)| id.env_id == env_id && matches!(info.status, WorkerStatus::Idle))
            .map(|(id, _)| id.clone())
    }

    pub async fn handle_task_completed(&mut self, req: TaskStateRequest) {
        if let Some(task) = req.task {
            // let task_id = TaskID::new(task.task_id.as_str());
            // if let Some(handle) = self.task_handles.remove(&task_id) {
            //     tokio::spawn(async move {
            //         let _ = handle.handle.await;
            //     });
            // }
            let worker_id = WorkerID::from_full_id(&task.worker_id);
            if let Some(worker) = self.registered_workers.get_mut(&worker_id) {
                worker.status = WorkerStatus::Idle;
            }
        }
    }

    /// Assign a task to a worker.
    ///
    /// This method will run in driver side.
    pub async fn assign_task_to_worker(
        &mut self,
        mut pending_task: PendingTask,
        tx_to_core: mpsc::UnboundedSender<RpcMessage>,
    ) {
        let tx_core = tx_to_core;
        let tx_notify = self.tx_core_notify.clone();
        if let Some(worker) = self.registered_workers.get_mut(&pending_task.worker_id) {
            worker.status = WorkerStatus::Busy;

            // Clone necessary data to use in the new task
            let mut worker_client = worker.worker_client.clone();
            let task_id = pending_task.task.task_id.clone();

            // Run the long-running operation in a new Tokio task
            let handle = tokio::task::spawn(async move {
                let streaming_result = pending_task.task.streaming_result;
                let rpc_task = pending_task
                    .task
                    .to_rpc_task_info(RPC_PROTOCOL_VERSION)
                    .await;

                let response = match rpc_task {
                    (Some(rpc_task_req), None) => {
                        if !streaming_result {
                            let res = worker_client.submit_task(rpc_task_req).await;
                            (Some(res), None)
                        } else {
                            let res = worker_client.to_stream_submit_task(rpc_task_req).await;
                            (None, Some(res))
                        }
                    }
                    (None, Some(stream)) => {
                        if !streaming_result {
                            let res = worker_client.un_stream_transform_submit_task(stream).await;
                            (Some(res), None)
                        } else {
                            let res = worker_client.stream_transform_submit_task(stream).await;
                            (None, Some(res))
                        }
                    }
                    _ => {
                        tracing::error!("Invalid task request");
                        return;
                    }
                };

                match response {
                    (Some(Ok(result)), None) => {
                        let task_state = result.into_inner();
                        if let Some(state) = task_state.clone().task {
                            let _ = pending_task
                                .tx
                                .send(Ok(TaskStateResult::TaskState(state.clone())));
                            let _ = tx_core.send(RpcMessage::TaskStateChange(TaskStateRequest {
                                version: RPC_PROTOCOL_VERSION,
                                task: Some(state),
                            }));
                        }
                    }
                    (Some(Err(e)), None) => {
                        pending_task.retry_times += 1;
                        let _ = pending_task
                            .tx
                            .send(Err(Error::InternalError(format!("{:?}", e))));
                    }
                    (None, Some(Ok(stream))) => {
                        let mut stream = stream.into_inner();
                        let (stream_tx, stream_rx) = mpsc::unbounded_channel();
                        let res_stream = UnboundedReceiverStream::new(stream_rx);

                        tracing::debug!("Stream task response: {:?}", res_stream);
                        tokio::spawn(async move {
                            tracing::debug!("Begin to receive stream task response");
                            let mut last_state = None;
                            while let Some(res) = stream.next().await {
                                match res {
                                    Ok(res) => match res.task {
                                        Some(task) => {
                                            // tracing::debug!("Stream task response: {:?}", res);
                                            let _ = stream_tx.send(Ok(task.clone()));
                                            last_state = Some(task);
                                        }
                                        None => {
                                            tracing::error!("Stream task response is empty");
                                        }
                                    },
                                    Err(e) => {
                                        tracing::error!("Stream task response error: {:?}", e);
                                        let _ = stream_tx.send(Err(TaskError::InternalError(
                                            format!("{:?}", e),
                                        )));
                                        break;
                                    }
                                }
                            }
                            let _ = stream_tx.send(Err(TaskError::StreamStopped));
                            let _ = tx_core.send(RpcMessage::TaskStateChange(TaskStateRequest {
                                version: RPC_PROTOCOL_VERSION,
                                task: last_state,
                            }));
                            tracing::debug!("Stream task response finished");
                        });
                        let _ = pending_task
                            .tx
                            .send(Ok(TaskStateResult::StreamTaskState(res_stream)));
                    }
                    (None, Some(Err(e))) => {
                        pending_task.retry_times += 1;
                        let _ = pending_task
                            .tx
                            .send(Err(Error::InternalError(format!("{:?}", e))));
                    }
                    _ => {
                        tracing::error!("Invalid task response");
                    }
                }
            });

            let task_handle = TaskHandle {
                task_id: task_id.clone(),
                worker_id: pending_task.worker_id.clone(),
                handle,
            };
            tracing::debug!("Task assigned: {:?}", task_id);
            self.task_id_to_worker_id
                .insert(task_id.clone(), pending_task.worker_id.clone());
            self.task_handles.insert(task_id, task_handle);
        } else {
            pending_task.retry_times += 1;
            let _ = tx_notify.send(NotifyMessage::RetryScheduleTask { pending_task });
        }
    }

    pub async fn stop_task(&mut self, task_id: TaskID, tx: ResultSender<(), Error>) {
        match (
            self.task_handles.remove(&task_id),
            self.task_id_to_worker_id.remove(&task_id),
        ) {
            (Some(handle), Some(worker_id)) => {
                if !handle.handle.is_finished() {
                    handle.handle.abort();
                }
                if let Some(worker) = self.registered_workers.get_mut(&worker_id) {
                    worker.status = WorkerStatus::Idle;
                    let mut worker_client = worker.worker_client.clone();
                    let req = TaskStopRequest {
                        version: RPC_PROTOCOL_VERSION,
                        task_id: task_id.as_str().to_string(),
                    };
                    tokio::task::spawn(async move {
                        match worker_client.stop_task(req).await {
                            Ok(_) => {
                                tracing::debug!("Task stopped: {:?}", task_id);
                                let _ = tx.send(Ok(()));
                            }
                            Err(e) => {
                                tracing::warn!(
                                    "Failed to stop task: {:?}, error: {:?}",
                                    task_id,
                                    e
                                );
                                let _ = tx.send(Err(Error::InternalError(format!("{:?}", e))));
                            }
                        }
                    });
                }
            }
            (Some(_), None) => {
                tracing::warn!("Task {} is not assigned to any worker", task_id.as_str());
                let _ = tx.send(Err(Error::InternalError(
                    "Task is not assigned to any worker".into(),
                )));
            }
            _ => {
                tracing::warn!("Task {} is not running", task_id.as_str());
                let _ = tx.send(Err(Error::InternalError("Task is not running".into())));
            }
        }
    }

    pub async fn launch_worker(
        &mut self,
        worker_id: WorkerID,
        lang: Language,
        env: EnvironmentConfigMessage,
    ) -> Result<(), Error> {
        // Check if the worker is already registered.
        if let Ok(_) = self._check_and_remove_registered_worker(&worker_id) {
            return Ok(());
        }
        // Check if the worker is already pending.
        if let Ok(_) = self._check_and_remove_pending_worker(&worker_id) {
            return Ok(());
        }
        let worker_public_host = if env.is_docker() {
            self.config.public_host.clone()
        } else {
            None
        };

        if let Some(start_command) = self.config.worker_start_commands.get(lang.as_str_name()) {
            let start_time = Local::now().timestamp_millis();
            let start_command = String::from(start_command);
            let default_node_id = worker_id.clone();
            let worker_cmd_args = WorkerStartCommand::new(
                start_command.as_str(),
                &self.config.parse_public_address("http")?,
                worker_id.full_id().as_str(),
                worker_public_host,
            )?;
            let tx = self.tx_core_notify.clone();
            // Run the start command in a blocking task separate from the main Tokio runtime
            self.pending_workers
                .insert(default_node_id.clone(), start_time);
            let worker_id = default_node_id.clone();
            let worker_port_start = self.config.worker_port_start;
            let worker_port_end = self.config.worker_port_end;
            let assigned_ports = Arc::clone(&self.assigned_ports);

            // let handle = tokio::runtime::Handle::current();
            let rt = self.runtime.clone();

            let _ = tokio::task::spawn_blocking(move || {
                let mut worker_cmd_args = worker_cmd_args;
                let exclude_ports: HashSet<u16> =
                    assigned_ports.lock().unwrap().iter().map(|p| *p).collect();

                if worker_cmd_args.worker_config.port.is_none() {
                    let port =
                        listen_available_port(worker_port_start, worker_port_end, exclude_ports)
                            .ok_or(Error::InternalError("No available port".into()))
                            .unwrap()
                            .0;
                    assigned_ports.lock().unwrap().insert(port);
                    worker_cmd_args.worker_config.port = Some(port);
                }
                let worker_port = worker_cmd_args.worker_config.port.unwrap();
                let cmd_args = worker_cmd_args.to_full_command();

                let cmd_str = cmd_args.join(" ");
                tracing::info!("Starting new worker: {}", cmd_str);

                match env {
                    EnvironmentConfigMessage::Local(local_config) => {
                        let mut builder = LocalEnvironmentBuilder::new().args(&cmd_args);
                        if let Some(envs) = local_config.envs {
                            builder = builder.envs(&envs);
                        }
                        builder = if let Some(working_dir) = local_config.working_dir {
                            builder.working_dir(working_dir.as_str())
                        } else {
                            builder
                        };
                        match builder.build().map_err(|e| Error::WorkerEnvError(e)) {
                            Ok(mut local_env) => {
                                rt.runtime.spawn(async move {
                                    let process = local_env
                                        .execute()
                                        .await
                                        .map_err(|e| Error::WorkerEnvError(e));
                                    let _ = tx.send(NotifyMessage::CreateWorkerResult {
                                        worker_id,
                                        process,
                                    });
                                });
                            }
                            Err(e) => {
                                let _ = tx.send(NotifyMessage::CreateWorkerResult {
                                    worker_id,
                                    process: Err(e),
                                });
                            }
                        }
                    }
                    EnvironmentConfigMessage::Docker(docker_config) => {
                        let mut builder =
                            DockerEnvironmentBuilder::new(docker_config.image.as_str())
                                .args(&cmd_args)
                                .port_binding(worker_port, worker_port)
                                .bridge("my_custom_lyric_bridge", true)
                                .mounts(docker_config.mounts.clone());
                        if let Some(envs) = docker_config.envs {
                            builder = builder.envs(&envs);
                        }
                        builder = if let Some(working_dir) = docker_config.working_dir {
                            builder.working_dir(working_dir.as_str())
                        } else {
                            builder
                        };
                        match builder.build().map_err(|e| Error::WorkerEnvError(e)) {
                            Ok(mut docker_env) => {
                                rt.runtime.spawn(async move {
                                    let process = docker_env
                                        .execute()
                                        .await
                                        .map_err(|e| Error::WorkerEnvError(e));
                                    let _ = tx.send(NotifyMessage::CreateWorkerResult {
                                        worker_id,
                                        process,
                                    });
                                });
                            }
                            Err(e) => {
                                let _ = tx.send(NotifyMessage::CreateWorkerResult {
                                    worker_id,
                                    process: Err(e),
                                });
                            }
                        }
                    }
                };
            });
        }
        Ok(())
    }

    pub async fn handle_register_worker(
        &mut self,
        req: RegisterWorkerRequest,
    ) -> Result<(), Error> {
        if let Some(worker) = req.worker {
            let worker_id = WorkerID::from_full_id(worker.worker_id.as_str());
            let worker_info = EnvWorkerInfo {
                status: WorkerStatus::Idle,
                worker_info: worker.clone(),
                // TODO: Connect to the worker in a separate task
                worker_client: connect_to_worker(&worker.address).await?,
                is_health: true,
                last_heartbeat: Local::now().timestamp_millis(),
            };
            self.pending_workers.remove(&worker_id);
            self.registered_workers.insert(worker_id, worker_info);
            Ok(())
        } else {
            Err(Error::InternalError("Worker info is empty".to_string()))
        }
    }

    pub async fn handle_create_worker_result(
        &mut self,
        worker_id: WorkerID,
        process: Result<Box<dyn ChildProcess>, Error>,
    ) {
        match process {
            Ok(mut process) => {
                tracing::info!("Worker started: {}", worker_id.full_id());
                let stdout = process.stdout().unwrap();
                let stderr = process.stderr().unwrap();
                let stdout_worker_id = worker_id.clone();
                let stderr_worker_id = worker_id.clone();
                let _ = self.runtime.runtime.spawn(async move {
                    let mut lines = stdout.lines().await;
                    while let Some(Ok(line)) = lines.next().await {
                        println!("[{:?}]: {:?}", stdout_worker_id.full_id(), line);
                    }
                });
                let _ = self.runtime.runtime.spawn(async move {
                    let mut lines = stderr.lines().await;
                    while let Some(Ok(line)) = lines.next().await {
                        println!("[{:?}-ERROR]: {:?}", stderr_worker_id.full_id(), line);
                    }
                });
                self.worker_handles.insert(worker_id, process);
            }
            Err(e) => {
                tracing::error!("Failed to start worker: {:?}", e);
                self.pending_workers.remove(&worker_id);
            }
        }
    }

    pub async fn cleanup_completed_tasks(&mut self) {
        // let completed_tasks: Vec<TaskID> = self
        //     .task_handles
        //     .iter()
        //     .filter(|(_, handle)| handle.handle.is_finished())
        //     .map(|(task_id, _)| task_id.clone())
        //     .collect();
        //
        // for task_id in completed_tasks {
        //     if let Some(handle) = self.task_handles.remove(&task_id) {
        //         let _ = handle.handle.await;
        //     }
        // }
    }

    fn _check_and_remove_registered_worker(&mut self, worker_id: &WorkerID) -> Result<(), Error> {
        if let Some(info) = self.registered_workers.get(worker_id) {
            return if info.is_health {
                Ok(())
            } else {
                self.registered_workers.remove(worker_id);
                Err(Error::WorkerError(format!(
                    "Worker {:?} is not health",
                    worker_id
                )))
            };
        }
        Err(Error::WorkerError(format!(
            "Worker {:?} is not registered",
            worker_id
        )))
    }

    fn _check_and_remove_pending_worker(&mut self, worker_id: &WorkerID) -> Result<(), Error> {
        if let Some(start_time) = self.pending_workers.get(worker_id) {
            if is_duration_exceeded(*start_time, self.launch_timeout) {
                self.pending_workers.remove(worker_id);
                return Err(Error::WorkerError(format!(
                    "Worker {:?} launch timeout",
                    worker_id
                )));
            }
            return Ok(());
        }
        Err(Error::WorkerError(format!(
            "Worker {:?} is not pending",
            worker_id
        )))
    }

    pub(crate) async fn stop(&mut self) {
        tracing::info!("Stopped task handles");
        for (_, handle) in self.task_handles.drain() {
            if !handle.handle.is_finished() {
                handle.handle.abort();
            }
        }
        tracing::info!(
            "Stopping worker environment manager, {} workers left",
            self.worker_handles.len()
        );
        let mut cleanup_handles = Vec::new();
        for (_, mut handle) in self.worker_handles.drain() {
            cleanup_handles.push(self.runtime.runtime.spawn(async move {
                let _ = handle.cleanup().await;
            }));
        }
        for handle in cleanup_handles {
            let _ = handle.await;
        }
        tracing::debug!("Stopped worker environment manager");
    }
}

fn calculate_duration(start_time: i64) -> i64 {
    let now = Local::now().timestamp_millis();
    now - start_time
}

fn is_duration_exceeded(start_time: i64, max_duration: Duration) -> bool {
    let now = Local::now().timestamp_millis();
    let elapsed = now - start_time;

    Duration::milliseconds(elapsed) > max_duration
}
