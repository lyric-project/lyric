use futures::StreamExt;
use lyric::{StreamTaskStateInfo, TaskDescription, TokioRuntime};
use lyric_rpc::task::{
    task_stream_submit_request, DataObject, ExecutionMode, ExecutionUnit, Language, TaskStateInfo,
};
use lyric_utils::err::{EnvError, TaskError};
use lyric_utils::prelude::Error;
use pyo3::exceptions::PyStopIteration;
use pyo3::{pyclass, pymethods, PyErr, PyObject, PyRef, PyRefMut, PyResult};
use serde::{Deserialize, Serialize};
use std::fmt::Debug;
use std::pin::Pin;
use std::sync::mpsc as std_mpsc;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio_stream::wrappers::UnboundedReceiverStream;
use tokio_stream::Stream;

pub async fn convert_stream<T, E>(
    mut stream: UnboundedReceiverStream<Result<T, E>>,
) -> std_mpsc::Receiver<T>
where
    T: Send + 'static + Debug,
    E: Send + 'static + Debug,
{
    let (tx, rx) = std_mpsc::channel();

    tracing::debug!("Begin convert_stream");
    tokio::spawn(async move {
        while let Some(item) = stream.next().await {
            match item {
                Ok(item) => {
                    tracing::debug!("convert_stream item: {:?}", item);
                    let _ = tx.send(item);
                }
                Err(e) => {
                    tracing::debug!("Error in stream: {:?}", e);
                    break;
                }
            }
        }
        tracing::debug!("All items sent");
    });

    tracing::debug!("End convert_stream");

    rx
}

#[pyclass]
#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct PyDataObject {
    #[pyo3(get, set)]
    pub object_id: String,
    #[pyo3(get, set)]
    pub format: i32,
    #[pyo3(get, set)]
    pub data: Vec<u8>,
}

#[pyclass]
#[derive(Clone, Debug)]
pub struct PyTaskOutputObject {
    pub data: PyDataObject,
    pub stderr: String,
    pub stdout: String,
}

#[pyclass]
#[derive(Clone, Debug)]
pub struct PyStreamDataObject {
    inner_stream: Arc<Mutex<UnboundedReceiverStream<PyDataObject>>>,
}

#[pyclass]
#[derive(Debug)]
pub struct PyStreamDataObjectIter {
    inner_stream: std_mpsc::Receiver<TaskStateInfo>,
}

#[pyclass]
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PyExecutionUnit {
    #[pyo3(get, set)]
    pub unit_id: String,
    #[pyo3(get, set)]
    pub language: i32,
    #[pyo3(get, set)]
    pub code: Option<PyDataObject>,
}

#[pyclass]
#[derive(Clone, Debug)]
pub struct PyTaskInfo {
    #[pyo3(get, set)]
    pub task_id: String,
    #[pyo3(get, set)]
    pub name: String,
    #[pyo3(get, set)]
    pub language: i32,
    #[pyo3(get, set)]
    pub exec_mode: i32,
    #[pyo3(get, set)]
    pub exec_unit: Option<PyExecutionUnit>,
    #[pyo3(get, set)]
    pub input: Option<PyDataObject>,
    #[pyo3(get, set)]
    pub stream_input: Option<PyStreamDataObject>,

    #[pyo3(get, set)]
    pub streaming_result: bool,
}

#[pyclass]
#[derive(Clone, Debug)]
pub struct PyTaskStateInfo {
    #[pyo3(get, set)]
    pub task_id: String,
    #[pyo3(get, set)]
    pub state: i32,
    #[pyo3(get, set)]
    pub start_time: i64,
    #[pyo3(get, set)]
    pub end_time: i64,
    #[pyo3(get, set)]
    pub worker_id: String,
    #[pyo3(get, set)]
    pub output: Option<PyDataObject>,
    #[pyo3(get, set)]
    pub exit_code: i32,
    #[pyo3(get, set)]
    pub stdout: String,
    #[pyo3(get, set)]
    pub stderr: String,
}

#[pymethods]
impl PyDataObject {
    #[new]
    #[pyo3(signature = (object_id, format, data))]
    fn new(object_id: String, format: i32, data: Vec<u8>) -> Self {
        PyDataObject {
            object_id,
            format,
            data,
        }
    }

    fn __str__(&self) -> String {
        format!(
            "PyDataObject(object_id={}, format={}",
            self.object_id, self.format
        )
    }
}

#[pymethods]
impl PyTaskOutputObject {
    #[new]
    #[pyo3(signature = (data, stderr, stdout))]
    fn new(data: PyDataObject, stderr: String, stdout: String) -> Self {
        PyTaskOutputObject {
            data,
            stderr,
            stdout,
        }
    }

    fn __str__(&self) -> String {
        format!(
            "PyTaskOutputObject(data={:?}, stderr={}, stdout={}",
            self.data, self.stderr, self.stdout
        )
    }
}

#[pymethods]
impl PyExecutionUnit {
    #[new]
    #[pyo3(signature = (unit_id, language, code=None))]
    fn new(unit_id: String, language: i32, code: Option<PyDataObject>) -> Self {
        PyExecutionUnit {
            unit_id,
            language,
            code,
        }
    }

    fn __str__(&self) -> String {
        format!(
            "PyExecutionUnit(unit_id={}, language={}",
            self.unit_id, self.language
        )
    }
}

#[pymethods]
impl PyTaskInfo {
    #[new]
    #[pyo3(signature = (task_id, name, language, exec_mode, exec_unit=None, input=None, stream_input=None, streaming_result=false))]
    fn new(
        task_id: String,
        name: String,
        language: i32,
        exec_mode: i32,
        exec_unit: Option<PyExecutionUnit>,
        input: Option<PyDataObject>,
        stream_input: Option<PyStreamDataObject>,
        streaming_result: bool,
    ) -> Self {
        PyTaskInfo {
            task_id,
            name,
            language,
            exec_mode,
            exec_unit,
            input,
            stream_input,
            streaming_result,
        }
    }

    fn __str__(&self) -> String {
        format!(
            "PyTaskInfo(task_id={}, name={}, language={}, exec_mode={}",
            self.task_id, self.name, self.language, self.exec_mode
        )
    }
}

impl PyStreamDataObjectIter {
    pub fn new(inner_stream: std_mpsc::Receiver<TaskStateInfo>) -> Self {
        PyStreamDataObjectIter { inner_stream }
    }

    pub async fn from_async_stream(inner_stream: StreamTaskStateInfo) -> Self {
        let inner_stream = convert_stream(inner_stream).await;
        PyStreamDataObjectIter { inner_stream }
    }
}

#[pymethods]
impl PyStreamDataObjectIter {
    pub fn __iter__(slf: PyRef<'_, Self>) -> PyRef<'_, Self> {
        slf
    }

    pub fn __next__(mut slf: PyRefMut<'_, Self>) -> Option<PyTaskStateInfo> {
        tracing::debug!("__next__");
        match slf.inner_stream.recv() {
            Ok(item) => {
                tracing::debug!("__next__ item: {:?}", item);
                Some(PyTaskStateInfo::from(item))
            }
            Err(e) => {
                tracing::debug!("__next__ error: {:?}", e);
                None
            }
        }
    }
}

impl TryFrom<PyTaskInfo> for TaskDescription {
    type Error = Error;
    fn try_from(py_task_info: PyTaskInfo) -> Result<Self, Self::Error> {
        let language = if let Some(exec_unit) = &py_task_info.exec_unit {
            Some(
                Language::try_from(exec_unit.language)
                    .map_err(|e| Error::InternalError(format!("Invalid language: {}", e)))?,
            )
        } else {
            None
        };
        let exec_mode = ExecutionMode::try_from(py_task_info.exec_mode)
            .map_err(|e| Error::InternalError(format!("Invalid exec mode: {}", e)))?;
        let exec_unit = py_task_info.exec_unit.map(|eu| ExecutionUnit::from(eu));
        let input = py_task_info.input.map(|i| DataObject::from(i));
        let stream_input = if let Some(stream_input) = py_task_info.stream_input {
            Some(UnboundedReceiverStream::try_from(stream_input)?)
        } else {
            None
        };
        Ok(TaskDescription::new(
            py_task_info.task_id,
            py_task_info.name,
            language,
            exec_mode,
            exec_unit,
            py_task_info.streaming_result,
            input,
            stream_input,
        ))
    }
}

pub(crate) trait AsyncTryFrom<T>: Sized {
    type Error;
    async fn async_try_from(t: T) -> Result<Self, Self::Error>;
}

enum TaskInfoOrData {
    Task(PyTaskInfo),
    Data(PyDataObject),
}

impl AsyncTryFrom<TaskDescription> for PyTaskInfo {
    type Error = Error;
    async fn async_try_from(task_info: TaskDescription) -> Result<Self, Self::Error> {
        let language = task_info
            .language
            .map(|l| l as i32)
            .unwrap_or(Language::Python as i32);
        let streaming_result = task_info.streaming_result;
        let (task, mut base_task, stream_task) = task_info
            .take_task_info(
                move |task_info| {
                    Ok(PyTaskInfo {
                        task_id: String::from(task_info.task_id),
                        name: task_info.name,
                        language,
                        exec_mode: task_info.exec_mode,
                        exec_unit: task_info.exec_unit.map(|eu| PyExecutionUnit::from(eu)),
                        input: task_info.input.map(|i| PyDataObject::from(i)),
                        stream_input: None,
                        streaming_result,
                    })
                },
                move |task_info, data| {
                    match (task_info, data) {
                        (_, Some(data)) => Ok(PyDataObject::from(data)),
                        // This error will be ignored
                        _ => Err(Error::InternalError(
                            "Invalid task info or data".to_string(),
                        )),
                    }
                },
            )
            .await;

        match (task, stream_task) {
            (Some(task), _) => Ok(task),
            (_, Some(stream_task)) => {
                if let Some(mut task) = base_task {
                    let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
                    tokio::spawn(async move {
                        let mut stream = stream_task;
                        while let Some(data) = stream.next().await {
                            let data = PyDataObject::from(data);
                            let _ = tx.send(data);
                        }
                    });
                    task.stream_input = Some(PyStreamDataObject {
                        inner_stream: Arc::new(Mutex::new(UnboundedReceiverStream::new(rx))),
                    });
                    Ok(task)
                } else {
                    Err(Error::InternalError(
                        "Invalid task info or data".to_string(),
                    ))
                }
            }
            _ => Err(Error::InternalError(
                "Invalid task info or data".to_string(),
            )),
        }
    }
}

impl From<PyDataObject> for DataObject {
    fn from(py_data_object: PyDataObject) -> Self {
        DataObject {
            object_id: py_data_object.object_id,
            format: py_data_object.format,
            data: py_data_object.data,
        }
    }
}
impl From<DataObject> for PyDataObject {
    fn from(data_object: DataObject) -> Self {
        PyDataObject {
            object_id: data_object.object_id,
            format: data_object.format,
            data: data_object.data,
        }
    }
}

impl From<PyExecutionUnit> for ExecutionUnit {
    fn from(py_execution_unit: PyExecutionUnit) -> Self {
        ExecutionUnit {
            unit_id: py_execution_unit.unit_id,
            language: py_execution_unit.language,
            code: py_execution_unit.code.map(|c| DataObject::from(c)),
        }
    }
}

impl From<ExecutionUnit> for PyExecutionUnit {
    fn from(execution_unit: ExecutionUnit) -> Self {
        Self {
            unit_id: execution_unit.unit_id,
            language: execution_unit.language,
            code: execution_unit.code.map(|c| PyDataObject::from(c)),
        }
    }
}

impl TryFrom<PyStreamDataObject> for UnboundedReceiverStream<DataObject> {
    type Error = Error;
    fn try_from(py_stream_data_object: PyStreamDataObject) -> Result<Self, Self::Error> {
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
        tokio::spawn(async move {
            let mut stream = py_stream_data_object.inner_stream;
            while let Some(data) = stream.lock().await.next().await {
                let data = DataObject::from(data);
                let _ = tx.send(data);
            }
        });
        Ok(UnboundedReceiverStream::new(rx))
    }
}

impl From<TaskStateInfo> for PyTaskStateInfo {
    fn from(task_state_info: TaskStateInfo) -> Self {
        PyTaskStateInfo {
            task_id: task_state_info.task_id,
            state: task_state_info.state as i32,
            start_time: task_state_info.start_time,
            end_time: task_state_info.end_time,
            worker_id: task_state_info.worker_id,
            output: task_state_info.output.map(|o| PyDataObject::from(o)),
            exit_code: task_state_info.exit_code,
            stdout: task_state_info.stdout,
            stderr: task_state_info.stderr,
        }
    }
}
