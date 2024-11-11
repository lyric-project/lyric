use thiserror::Error;

#[derive(Error, Debug)]
pub enum WasmError {
    #[error(transparent)]
    IOError(#[from] std::io::Error),

    #[error(transparent)]
    RuntimeError(#[from] anyhow::Error),

    #[error("Wasm runtime stopped, msg: `{0}`")]
    RuntimeStopped(String),

    #[error("Failed to create handler: {0}")]
    HandlerCreationError(String),

    #[error("Failed to create component: {0}")]
    DependencyNotFound(String),
}
