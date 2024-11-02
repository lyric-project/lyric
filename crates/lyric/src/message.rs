use crate::env::EnvironmentConfigMessage;
use crate::task::{TaskDescription, TaskID};
use crate::worker::WorkerID;
use lyric_rpc::task::{RegisterWorkerRequest, StopWorkerRequest, TaskStateInfo, TaskStateRequest};
use lyric_utils::prelude::{Error, TaskError};
use tokio::sync::oneshot;
use uuid::Uuid;

use tokio_stream::wrappers::UnboundedReceiverStream;
use tokio_stream::{Stream, StreamExt};

pub type ResultSender<T, E> = oneshot::Sender<Result<T, E>>;
pub type StreamTaskStateInfo = UnboundedReceiverStream<Result<TaskStateInfo, TaskError>>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct EnvId(Uuid);

impl EnvId {
    pub fn new() -> Self {
        EnvId(Uuid::new_v4())
    }
}

#[derive(Debug)]
pub enum TaskStateResult {
    TaskState(TaskStateInfo),
    StreamTaskState(StreamTaskStateInfo),
}

#[derive(Debug)]
pub(crate) enum RpcMessage {
    SubmitTask {
        rpc: TaskDescription,
        tx: ResultSender<TaskStateResult, Error>,
        /// The environment configuration for the task.
        env: Option<EnvironmentConfigMessage>,
    },
    StopTask {
        task_id: TaskID,
        tx: ResultSender<(), Error>,
    },
    TaskStateChange(TaskStateRequest),
    RegisterWorker(RegisterWorkerRequest),
    StopWorker(StopWorkerRequest),
}

#[derive(Debug)]
pub enum LangWorkerMessage {
    SubmitTask {
        rpc: TaskDescription,
        tx: ResultSender<TaskStateResult, Error>,
        worker_id: String,
    },
    SubmitLaunchComponent {
        rpc: TaskDescription,
        tx: ResultSender<TaskStateResult, Error>,
        worker_id: String,
    },
    StopComponentTask {
        task_id: TaskID,
        tx: ResultSender<(), Error>,
        worker_id: String,
    },
}

#[derive(Debug)]
pub(crate) enum TriggerScheduleEvent {
    TaskStateChange(TaskStateRequest),
    RegisterWorker(RegisterWorkerRequest),
    SubmitTask(TaskDescription),
    TimeInterval,
}

#[derive(Debug)]
pub(crate) enum NotifyMessage {
    TriggerSchedule {
        event: TriggerScheduleEvent,
        timestamp: i64,
    },
    CreateWorkerResult {
        // message: String,
        worker_id: WorkerID,
        // handle: WorkerHandle,
        process: Result<Box<dyn crate::env::ChildProcess>, Error>,
    },
    RetryScheduleTask {
        pending_task: PendingTask,
    },
}

impl NotifyMessage {
    pub(crate) fn trace_info(&self) -> String {
        match self {
            NotifyMessage::TriggerSchedule { event, timestamp } => {
                format!("TriggerSchedule: {:?}, timestamp: {}", event, timestamp)
            }
            NotifyMessage::CreateWorkerResult {
                worker_id,
                process: _,
            } => {
                format!("CreateWorkerResult: worker_id: {}", worker_id.full_id())
            }
            NotifyMessage::RetryScheduleTask { pending_task } => {
                format!("RetryScheduleTask: worker_id: {:?}", pending_task.worker_id)
            }
        }
    }
}

impl RpcMessage {
    pub(crate) fn trace_info(&self) -> String {
        match self {
            RpcMessage::SubmitTask { rpc, tx: _, env: _ } => {
                format!("SubmitTask: {:?}", rpc.task_id)
            }
            RpcMessage::StopTask { task_id, tx: _ } => {
                format!("StopTask: {:?}", task_id)
            }
            RpcMessage::TaskStateChange(_) => {
                format!("TaskStateChange")
            }
            RpcMessage::RegisterWorker(_) => {
                format!("RegisterWorker")
            }
            RpcMessage::StopWorker(_) => {
                format!("StopWorker")
            }
        }
    }
}

#[derive(Debug)]
pub(crate) struct PendingTask {
    pub(crate) task: TaskDescription,
    pub(crate) tx: ResultSender<TaskStateResult, Error>,
    pub(crate) env: EnvironmentConfigMessage,
    pub(crate) worker_id: WorkerID,
    pub(crate) retry_times: u16,
}
