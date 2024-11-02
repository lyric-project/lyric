use futures_util::StreamExt;
use lyric_rpc::task::{
    task_stream_submit_request, DataObject, ExecutionMode, ExecutionUnit, Language, TaskInfo,
    TaskStreamSubmitRequest, TaskSubmitRequest,
};
use lyric_utils::prelude::Error;
use std::fmt;
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex};
use tokio_stream::wrappers::UnboundedReceiverStream;
use tokio_stream::Stream;

type ExecutionUnitType = Arc<Mutex<Option<ExecutionUnit>>>;
type DataObjectType = Arc<Mutex<Option<DataObject>>>;
type StreamDataObjectType = Arc<Mutex<Option<UnboundedReceiverStream<DataObject>>>>;

#[derive(Debug, Clone)]
pub enum WrappedData {
    Data(DataObjectType),
    StreamData(StreamDataObjectType),
}

#[derive(Clone)]
pub struct TaskDescription {
    pub task_id: TaskID,
    pub name: String,
    pub exec_unit: ExecutionUnitType,
    // Split language from exec_unit, for easy access
    pub language: Option<Language>,
    pub exec_mode: ExecutionMode,
    pub streaming_result: bool,
    pub input: Option<WrappedData>,
}

impl TaskDescription {
    pub fn new<T>(
        task_id: T,
        name: T,
        language: Option<Language>,
        exec_mode: ExecutionMode,
        exec_unit: Option<ExecutionUnit>,
        streaming_result: bool,
        data: Option<DataObject>,
        stream_data: Option<UnboundedReceiverStream<DataObject>>,
    ) -> Self
    where
        T: Into<String>,
    {
        let input = if let Some(data) = data {
            Some(WrappedData::Data(Arc::new(Mutex::new(Some(data)))))
        } else if let Some(stream_data) = stream_data {
            Some(WrappedData::StreamData(Arc::new(Mutex::new(Some(
                stream_data,
            )))))
        } else {
            tracing::warn!("Task input is empty");
            None
        };
        Self {
            task_id: TaskID::new(task_id),
            name: name.into(),
            exec_unit: Arc::new(Mutex::new(exec_unit)),
            language,
            exec_mode,
            streaming_result,
            input,
        }
    }

    pub async fn to_rpc_task_info(
        &self,
        version: i32,
    ) -> (
        Option<TaskSubmitRequest>,
        Option<impl Stream<Item = TaskStreamSubmitRequest>>,
    ) {
        let (task, base_task, stream_task) = self
            .take_task_info(
                move |task_info| {
                    Ok(TaskSubmitRequest {
                        version,
                        task: Some(task_info),
                    })
                },
                move |task_info, data| {
                    Ok(TaskStreamSubmitRequest {
                        version,
                        request: match (task_info, data) {
                            (Some(task_info), _) => {
                                Some(task_stream_submit_request::Request::Task(task_info))
                            }
                            (_, Some(data)) => {
                                Some(task_stream_submit_request::Request::Input(data))
                            }
                            _ => None,
                        },
                    })
                },
            )
            .await;
        (task, stream_task)
    }

    pub async fn take_task_info<F, SF, T, S>(
        &self,
        f: F,
        sf: SF,
    ) -> (Option<T>, Option<T>, Option<impl Stream<Item = S>>)
    where
        F: Fn(TaskInfo) -> Result<T, Error> + Send,
        SF: Fn(Option<TaskInfo>, Option<DataObject>) -> Result<S, Error> + Send + 'static + Clone,
        T: Send,
        S: Send + 'static,
    {
        let exec_unit = {
            let eu = self.exec_unit.lock().await;
            eu.as_ref().cloned()
        };

        let base_task = TaskInfo {
            task_id: self.task_id.0.clone(),
            name: self.name.clone(),
            exec_unit,
            exec_mode: self.exec_mode.clone() as i32,
            input: None,
        };

        let result_base_task = f(base_task.clone()).ok();
        let no_stream_task = f(base_task.clone()).ok();

        match &self.input {
            None => {
                tracing::debug!("Task input is empty: {:?}", base_task);
                (no_stream_task, result_base_task, None)
            }
            Some(WrappedData::Data(data)) => {
                let mut data = data.lock().await.take();
                let mut task_info = base_task;
                task_info.input = data;
                tracing::debug!("Task input data is not streaming: {:?}", task_info);
                if let Some(data) = f(task_info).ok() {
                    (Some(data), result_base_task, None)
                } else {
                    (no_stream_task, result_base_task, None)
                }
            }
            Some(WrappedData::StreamData(stream_data)) => {
                let stream_data = stream_data.lock().await.take();
                if let Some(stream_data) = stream_data {
                    let sf = sf.clone();
                    let (tx, rx) = mpsc::unbounded_channel();
                    tokio::spawn(async move {
                        if let Ok(t) = sf(Some(base_task), None) {
                            let _ = tx.send(t);
                        }
                        let mut stream = Box::pin(stream_data);
                        while let Some(data) = stream.next().await {
                            if let Ok(t) = sf(None, Some(data)) {
                                let _ = tx.send(t);
                            }
                        }
                    });

                    (
                        None,
                        result_base_task,
                        Some(UnboundedReceiverStream::new(rx)),
                    )
                } else {
                    (None, result_base_task, None)
                }
            }
        }
    }

    pub(crate) fn trace_info(&self) -> String {
        format!(
            "TaskDescription: task_id: {}, name: {}, exec_mode: {:?}, language: {:?}",
            self.task_id.0, self.name, self.exec_mode, self.language
        )
    }
}

impl fmt::Debug for TaskDescription {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TaskDescription")
            .field("task_id", &self.task_id)
            .field("name", &self.name)
            .field("language", &self.language)
            .field("exec_mode", &self.exec_mode)
            .finish()
    }
}

#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct TaskID(String);

impl TaskID {
    pub fn new<T>(id: T) -> Self
    where
        T: Into<String>,
    {
        TaskID(id.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<TaskID> for String {
    fn from(task_id: TaskID) -> Self {
        task_id.0
    }
}

impl TryFrom<TaskInfo> for TaskDescription {
    type Error = Error;

    fn try_from(task_info: TaskInfo) -> Result<Self, Self::Error> {
        let language = if let Some(exec_unit) = &task_info.exec_unit {
            Some(
                Language::try_from(exec_unit.language)
                    .map_err(|e| Error::InternalError(format!("Invalid language: {}", e)))?,
            )
        } else {
            None
        };
        let exec_mode = ExecutionMode::try_from(task_info.exec_mode)
            .map_err(|e| Error::InternalError(format!("Invalid exec mode: {}", e)))?;
        Ok(Self::new(
            task_info.task_id,
            task_info.name,
            language,
            exec_mode,
            task_info.exec_unit,
            false,
            task_info.input,
            None,
        ))
    }
}

impl From<String> for TaskID {
    fn from(id: String) -> Self {
        TaskID(id)
    }
}
