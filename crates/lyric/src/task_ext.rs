use crate::{Lyric, TaskDescription, TokioRuntime, WrappedData};
use async_trait::async_trait;
use lyric_rpc::task::{DataObject, ExecutionUnit, TaskStateInfo};
use lyric_utils::prelude::{Error, TaskError};
use lyric_wasm_runtime::Handler;
use serde_json::Value;
use std::sync::Arc;
use wrpc_transport::Invoke;

pub type ClientType<T> = wrpc_transport::tcp::Client<T>;

pub struct TaskHandle<T>
where
    T: Clone + Send + Sync + 'static + tokio::net::ToSocketAddrs,
{
    handler: Handler<ClientType<T>>,
    pub lyric: Lyric,
    pub runtime: TokioRuntime,
    pub task_description: Arc<TaskDescription>,
    is_stop: bool,
}

impl<T> TaskHandle<T>
where
    T: Clone + Send + Sync + 'static + tokio::net::ToSocketAddrs,
{
    pub fn new(
        lyric: Lyric,
        runtime: TokioRuntime,
        task_description: Arc<TaskDescription>,
        handler: Handler<ClientType<T>>,
    ) -> Self {
        Self {
            handler,
            lyric,
            runtime,
            task_description,
            is_stop: false,
        }
    }
    pub fn copy_handler(&self) -> Handler<ClientType<T>> {
        self.handler.clone()
    }

    pub async fn stop(&mut self) -> Result<(), Error> {
        tracing::info!("Stopping task: {:?}", self.task_description.task_id);
        if self.is_stop {
            return Ok(());
        }
        let task_id = self.task_description.task_id.clone();
        stop_task(self.lyric.clone(), task_id.into()).await?;
        self.is_stop = true;
        Ok(())
    }
}

impl<T> Drop for TaskHandle<T>
where
    T: Clone + Send + Sync + 'static + tokio::net::ToSocketAddrs,
{
    fn drop(&mut self) {
        tracing::info!("Dropping task handle: {:?}", self.task_description.task_id);
        if !self.is_stop {
            let task_id = self.task_description.task_id.clone();
            let lyric = self.lyric.clone();
            let _ = self
                .runtime
                .runtime
                .block_on(stop_task(lyric, task_id.into()));
        }
    }
}

async fn stop_task(lyric: Lyric, task_id: String) -> Result<(), Error> {
    lyric.stop_task(task_id).await
}

pub trait TaskBytesExt {
    fn task_bytes(self) -> Result<(String, Vec<u8>), Error>;
}

impl TaskBytesExt for Option<ExecutionUnit> {
    fn task_bytes(self) -> Result<(String, Vec<u8>), Error> {
        match self {
            Some(ExecutionUnit {
                unit_id,
                language,
                code,
            }) => {
                match code {
                    Some(DataObject {
                        object_id,
                        format,
                        data,
                    }) => {
                        // Parse code from Vec<u8>
                        Ok((unit_id, data))
                    }
                    _ => Err(Error::InternalError(
                        "No code data object found".to_string(),
                    )),
                }
            }
            _ => Err(Error::InternalError("No execution unit found".to_string())),
        }
    }
}

pub trait TaskHandlerExt {
    type Client: Invoke + Clone + 'static;

    fn task_handler(&mut self, component_id: String) -> Result<Handler<Self::Client>, Error>;
}

impl TaskHandlerExt for TaskStateInfo {
    type Client = ClientType<String>;

    fn task_handler(&mut self, component_id: String) -> Result<Handler<Self::Client>, Error> {
        match self.output.take() {
            Some(DataObject {
                object_id,
                format,
                data,
            }) => {
                // Parse address from Vec<u8>
                let address = String::from_utf8(data)
                    .map_err(|_| Error::InternalError("Failed to parse address".to_string()))?;
                tracing::info!("Connecting to component: {}", address);
                let client = wrpc_transport::tcp::Client::from(address.clone());
                Ok(Handler {
                    component_id: Arc::from(component_id),
                    client: Arc::new(client),
                })
            }
            _ => Err(Error::InternalError(
                "No output data object found".to_string(),
            )),
        }
    }
}

#[async_trait]
pub trait TaskComponentIDExt {
    async fn task_component_id(&self) -> Result<String, Error>;
}

#[async_trait]
impl TaskComponentIDExt for TaskDescription {
    async fn task_component_id(&self) -> Result<String, Error> {
        self.task_id
            .as_str()
            .parse()
            .map_err(|_| Error::InternalError("Failed to parse task_id".to_string()))
    }
}

#[async_trait]
pub trait TaskInputExt {
    async fn input_data(&mut self) -> Option<DataObject>;
}

#[async_trait]
impl TaskInputExt for TaskDescription {
    async fn input_data(&mut self) -> Option<DataObject> {
        match &self.input {
            Some(WrappedData::Data(data)) => data.lock().await.take(),
            _ => None,
        }
    }
}

/// Deserialize a messagepack buffer to JSON string buffer.
///
pub trait MsgpackDeserializeExt {
    fn deserialize(self) -> Result<Vec<u8>, TaskError>;
}

impl<T> MsgpackDeserializeExt for T
where
    T: Into<Vec<u8>>,
{
    fn deserialize(self) -> Result<Vec<u8>, TaskError> {
        match rmp_serde::from_slice::<Value>(self.into().as_slice()) {
            Ok(value) => match serde_json::to_vec(&value) {
                Ok(json) => Ok(json),
                Err(e) => Err(TaskError::DataParseError(format!(
                    "JSON encoding error: {}",
                    e
                ))),
            },
            Err(e) => Err(TaskError::DataParseError(format!(
                "Deserialization error: {}",
                e
            ))),
        }
    }
}

pub trait MsgpackSerializeExt {
    fn serialize(self) -> Result<Vec<u8>, TaskError>;
}

impl<T> MsgpackSerializeExt for T
where
    T: Into<Vec<u8>>,
{
    fn serialize(self) -> Result<Vec<u8>, TaskError> {
        let value: Value = match serde_json::from_slice(self.into().as_slice()) {
            Ok(v) => v,
            Err(e) => return Err(TaskError::DataParseError(format!("Invalid JSON: {}", e))),
        };

        match rmp_serde::to_vec(&value) {
            Ok(buf) => Ok(buf),
            Err(e) => Err(TaskError::DataParseError(format!(
                "Serialization error: {}",
                e
            ))),
        }
    }
}
