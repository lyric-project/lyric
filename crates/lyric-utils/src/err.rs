use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
    #[error("Task not found, task_id: `{0}`")]
    TaskNotFound(String),

    #[error("Task detail not found, task_id: `{0}`")]
    TaskDetailNotFound(String),

    #[error("Task state not found, task_id: `{0}`")]
    UnsupportedTaskType(String),

    #[error("Environment not found, env_id: `{0}`")]
    EnvironmentNotFound(String),

    #[error("Execute task error, msg: `{0}`")]
    ExecutionError(String),

    #[error("IO error, msg: `{0}`")]
    IoError(#[from] std::io::Error),

    #[error("Connect to worker error, msg: `{0}`")]
    ConnectWorker(String),

    #[error("Connect to driver error, msg: `{0}`")]
    ConnectDriver(String),

    #[error("Internal error: {0}")]
    InternalError(String),

    #[error("panicked")]
    Panicked,

    #[error("Core stopped, msg: `{0}`")]
    CoreStopped(String),

    #[error(transparent)]
    APIError(#[from] anyhow::Error),

    #[error("Worker error, msg: `{0}`")]
    WorkerError(String),

    #[error("Worker env error, msg: `{0}`")]
    WorkerEnvError(#[from] EnvError),
}

#[derive(Error, Debug)]
pub enum EnvError {
    #[error("Launch environment error, msg: `{0}`")]
    LaunchEnvError(String),

    #[error("IO error, msg: `{0}`")]
    IoError(#[from] std::io::Error),
}

#[derive(Error, Debug)]
pub enum TaskError {
    #[error("Task execute error, msg: `{0}`")]
    TaskExecuteError(String),

    #[error("Internal error: {0}")]
    InternalError(String),

    #[error("TaskStreamStopped")]
    StreamStopped,

    #[error("Data parse error: {0}")]
    DataParseError(String),
}
