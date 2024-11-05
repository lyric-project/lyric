use crate::config::{PyConfig, PyDriverConfig, PyWorkerConfig};
use crate::env::PyEnvironmentConfig;
use crate::error::pyerr_to_lyric_err;
use crate::handle::PyTaskHandle;
use crate::task::{
    AsyncTryFrom, PyStreamDataObjectIter, PyTaskInfo, PyTaskOutputObject, PyTaskStateInfo,
};
use crate::types::PyUnboundedReceiverStream;
use chrono::Local;
use futures::StreamExt;
use lyric::task_ext::{TaskComponentIDExt, TaskHandle, TaskHandlerExt, TaskInputExt};
use lyric::{
    Config, DriverConfig, EnvironmentConfigMessage, LangWorkerMessage, Lyric, ResultSender,
    TaskDescription, TaskStateResult, TokioRuntime, WorkerConfig,
};
use lyric_rpc::task::{
    DataFormat, DataObject, ExecutionUnit, Language, TaskInfo, TaskState, TaskStateInfo,
};
use lyric_utils::err::TaskError;
use lyric_utils::prelude::Error;
use lyric_wasm_runtime::capability::wrpc::lyric::task::types;
use pyo3::prelude::*;
use pyo3::types::{PyDict, PyIterator, PyList, PyString};
use std::borrow::Cow;
use std::panic;
use std::sync::Arc;
use std::sync::Mutex as StdMutex;
use tokio::sync::mpsc::unbounded_channel;
use tokio::sync::{mpsc, oneshot, Mutex, Notify};
use tokio_stream::wrappers::UnboundedReceiverStream;
use tracing;

struct PyLyricInner {
    callback: Arc<StdMutex<Option<PyObject>>>,
    tx_shutdown: oneshot::Sender<()>,
    notify: Arc<Notify>,
}

#[pyclass]
#[derive(Clone)]
pub struct PyLyric {
    pub lyric: Lyric,
    inner: Arc<StdMutex<PyLyricInner>>,
    runtime: TokioRuntime,
    lang_worker_handle: Arc<StdMutex<Option<tokio::task::JoinHandle<()>>>>,
}

#[pymethods]
impl PyLyric {
    #[new]
    fn new(config: PyConfig) -> Self {
        let eventloop_worker_threads = config.eventloop_worker_threads.unwrap_or(4);
        let runtime = TokioRuntime::new(eventloop_worker_threads).unwrap();
        let (tx_lang_worker, rx_lang_worker) = mpsc::unbounded_channel();
        let (tx_shutdown, rx_shutdown) = oneshot::channel();

        let config = Config::from(config);

        let lyric = Lyric::new(runtime.clone(), tx_lang_worker, config).unwrap();
        let notify = Arc::new(Notify::new());

        let mut inner = Arc::new(StdMutex::new(PyLyricInner {
            callback: Arc::new(StdMutex::new(None)),
            tx_shutdown,
            notify: notify.clone(),
        }));

        let new_inner = Arc::clone(&inner);
        let new_runtime = runtime.clone();
        let lyric_clone = lyric.clone();
        let lang_worker_handle = runtime.runtime.spawn(async move {
            let _ = language_worker_task_loop(
                lyric_clone,
                new_inner.clone(),
                new_runtime,
                rx_lang_worker,
                rx_shutdown,
            )
            .await;
            new_inner.lock().unwrap().notify.notify_one();
        });
        Self {
            lyric,
            inner,
            runtime,
            lang_worker_handle: Arc::new(StdMutex::new(Some(lang_worker_handle))),
        }
    }

    fn start_worker(&self, config: PyWorkerConfig) -> PyResult<()> {
        // pyo3_pylogger::register("lyric");
        self.lyric
            .start_worker(WorkerConfig::from(config))
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))?;
        Ok(())
    }
    fn start_driver(&self, config: PyDriverConfig) -> PyResult<()> {
        self.lyric
            .start_driver(DriverConfig::from(config))
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))?;
        Ok(())
    }
    fn stop(&mut self) -> PyResult<()> {
        let inner = self.inner.lock().unwrap();
        inner.notify.notify_one();
        tracing::info!("Notified lyric core to stop");
        self.lyric.stop();
        if let Some(handle) = self.lang_worker_handle.lock().unwrap().take() {
            if !handle.is_finished() {
                handle.abort();
            }
        }
        tracing::info!("Lyric core stopped");
        Ok(())
    }

    fn set_callback(&self, callback: PyObject) -> PyResult<()> {
        let inner = self.inner.clone();
        inner
            .lock()
            .unwrap()
            .callback
            .lock()
            .unwrap()
            .replace(callback);
        Ok(())
    }

    #[pyo3(signature = (py_task_info, environment_config=None))]
    async fn submit_task(
        &self,
        py_task_info: PyTaskInfo,
        environment_config: Option<PyEnvironmentConfig>,
    ) -> PyResult<PyTaskHandle> {
        let task_info: TaskDescription = TaskDescription::try_from(py_task_info)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))?;
        let lyric = self.lyric.clone();
        let env = if let Some(env_config) = environment_config {
            Some(EnvironmentConfigMessage::from(env_config))
        } else {
            None
        };
        let copy_task_info = task_info.clone();
        let component_id = task_info
            .task_component_id()
            .await
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))?;

        let task_state_res = lyric
            .submit_task(task_info, env)
            .await
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))?;
        match task_state_res {
            TaskStateResult::TaskState(mut state_info) => {
                if state_info.exit_code != 0 {
                    tracing::error!(
                        "Task failed with exit code: {}, stderr: {}",
                        state_info.exit_code,
                        state_info.stderr
                    );
                    return Err(PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(format!(
                        "Task failed with exit code: {}, stderr: {}",
                        state_info.exit_code, state_info.stderr
                    )));
                }
                let handler = state_info.task_handler(component_id).unwrap();
                let th = TaskHandle::new(
                    self.lyric.clone(),
                    self.runtime.clone(),
                    Arc::new(copy_task_info),
                    handler,
                );
                Ok(PyTaskHandle {
                    inner: Arc::new(Mutex::new(th)),
                })
            }
            TaskStateResult::StreamTaskState(stream_state_info) => {
                tracing::debug!("Received stream state info: {:?}", stream_state_info);
                Err(PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(
                    "Stream task state not supported",
                ))
            }
        }
    }

    #[pyo3(name = "submit_task_async", signature = (py_task_info, callback, environment_config=None))]
    async fn submit_task_async(
        &self,
        py_task_info: PyTaskInfo,
        callback: PyObject,
        environment_config: Option<PyEnvironmentConfig>,
    ) -> PyResult<String> {
        tracing::debug!("submit_task_async, py_task_info: {:?}", py_task_info);
        let mut task_info: TaskDescription = TaskDescription::try_from(py_task_info)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))?;
        let env = if let Some(env_config) = environment_config {
            Some(EnvironmentConfigMessage::from(env_config))
        } else {
            None
        };
        tracing::debug!("Submitting task: {:?}", task_info);
        let rt = self.runtime.clone();
        let callback = Arc::new(callback);
        let task_component_id = task_info
            .task_component_id()
            .await
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))?;
        let input_data = task_info.input_data().await;
        self.lyric
            .submit_task_with_callback(task_info, env, move |result| {
                let rt = rt.clone();
                let callback = callback.clone();
                let component_id = task_component_id.clone();
                let task_input = input_data.clone();
                async move {
                    use lyric_wasm_runtime::capability::wrpc::lyric::task::{
                        binary_task, interpreter_task, types,
                    };
                    use lyric_wasm_runtime::Handler;

                    match result {
                        Ok(TaskStateResult::TaskState(mut state_info)) => {
                            if state_info.exit_code != 0 {
                                tracing::error!(
                                    "Task failed with exit code: {}, stderr: {}",
                                    state_info.exit_code,
                                    state_info.stderr
                                );
                            }
                            let handler = state_info.task_handler(component_id).unwrap();
                            let output = if let Some(task_input) = task_input {
                                let req = types::BinaryRequest {
                                    resources: None,
                                    protocol: 1_u32,
                                    data: task_input.data.into(),
                                };
                                let res = binary_task::run(&handler, (), &req).await;
                                let bytes =
                                    if let Ok(Ok(types::BinaryResponse { protocol, data })) = res {
                                        data.into()
                                    } else {
                                        "Run task wasm worker failed, no response found"
                                            .as_bytes()
                                            .to_vec()
                                    };
                                DataObject {
                                    object_id: "".to_string(),
                                    format: DataFormat::Pickle as i32,
                                    data: bytes,
                                }
                            } else {
                                let req = types::InterpreterRequest {
                                    resources: None,
                                    protocol: 1_u32,
                                    lang: "rust".to_string(),
                                    code: "hello".to_string(),
                                };
                                let res = interpreter_task::run(&handler, (), &req).await;
                                tracing::info!("Interpreter task result: {:?}", res);
                                DataObject {
                                    object_id: "".to_string(),
                                    format: DataFormat::Raw as i32,
                                    data: "Run task in wasm worker successfully"
                                        .as_bytes()
                                        .to_vec(),
                                }
                            };
                            state_info.output = Some(output);
                            let py_result = PyTaskStateInfo::from(state_info);
                            rt.runtime.spawn_blocking(move || {
                                Python::with_gil(|py| callback.call1(py, (py_result,)))
                            });
                        }
                        _ => {
                            tracing::error!("Error calling Python callback");
                            rt.runtime.spawn_blocking(move || {
                                Python::with_gil(|py| callback.call1(py, (1,)))
                            });
                        }
                    };
                }
            })
            .await
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    fn join(&self) -> PyResult<()> {
        let notify = self.inner.lock().unwrap().notify.clone();

        Python::with_gil(|py| {
            py.allow_threads(|| {
                // Release GIL and wait for the notify to be triggered
                self.runtime.runtime.block_on(async {
                    notify.notified().await;
                });
            })
        });

        Ok(())
    }
}

async fn language_worker_task_loop(
    lyric: Lyric,
    inner: Arc<StdMutex<PyLyricInner>>,
    runtime: TokioRuntime,
    mut rx_lang_worker: mpsc::UnboundedReceiver<LangWorkerMessage>,
    mut rx_shutdown: oneshot::Receiver<()>,
) -> Result<(), Error> {
    loop {
        tokio::select! {
            biased;

            _ = &mut rx_shutdown => {
                return Err(Error::CoreStopped("recv from rx_shutdown".to_string()));
            }
            msg = rx_lang_worker.recv() => {
                tracing::info!("Received lang worker message: {:?}", msg);
                let inner = Arc::clone(&inner);
                match msg {
                    Some(msg) => {
                        _handle_submit_with_type(lyric.clone(), inner.clone(), runtime.clone(), msg).await?;
                    }
                    None => {
                        tracing::debug!("Failed to receive message from lang worker");
                        return Err(Error::InternalError("Failed to receive message from lang worker".to_string()));
                    }
                }

            }
        }
    }
}

#[tracing::instrument(level = "debug", skip_all)]
async fn _handle_submit_with_type(
    lyric: Lyric,
    inner: Arc<StdMutex<PyLyricInner>>,
    runtime: TokioRuntime,
    msg: LangWorkerMessage,
) -> Result<(), Error> {
    match msg {
        LangWorkerMessage::SubmitTask { rpc, tx, worker_id } => {
            tracing::debug!("Lang worker Received SubmitTask message: {:?}", rpc);
            let py_task_info = PyTaskInfo::async_try_from(rpc.clone()).await?;
            let task_id = String::from(rpc.task_id);
            let start_time = Local::now().timestamp_millis();
            let callback = inner.lock().unwrap().callback.clone();
            runtime.runtime.spawn_blocking(move || {
                let res = Python::with_gil(|py| {
                    // Maybe deadlock here
                    if let Some(callback_fn) = callback.lock().unwrap().as_ref() {
                        callback_fn.call1(py, (py_task_info,))
                    } else {
                        eprintln!("No callback function set");
                        Err(PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(
                            "No callback function set",
                        ))
                    }
                });
                match res {
                    Ok(result) => {
                        // Handle the result of the callback function
                        if let Err(e) =
                            process_callback_result(result, tx, task_id, worker_id, start_time)
                        {
                            eprintln!("Error processing callback result: {:?}", e);
                        }
                    }
                    Err(e) => {
                        eprintln!("Error calling callback function: {:?}", e);
                        let end_time = Local::now().timestamp_millis();
                        let state_info = TaskStateInfo {
                            task_id: task_id.clone(),
                            state: TaskState::TaskFailed as i32,
                            start_time,
                            end_time,
                            worker_id,
                            output: None,
                            exit_code: 1,
                            stdout: "".to_string(),
                            stderr: format!("Error calling callback: {}", e),
                        };
                        let _ = tx.send(Ok(TaskStateResult::TaskState(state_info)));
                    }
                }
            });
        }
        LangWorkerMessage::SubmitLaunchComponent { rpc, tx, worker_id } => {
            // The wasm bytes to launch a new component
            let exec_unit = rpc.exec_unit.lock().await.take();
            let task_id = rpc.task_id.clone();
            let mut state_info = TaskStateInfo {
                task_id: String::from(task_id.clone()),
                state: TaskState::TaskRunning as i32,
                start_time: Local::now().timestamp_millis(),
                end_time: 0,
                worker_id: worker_id.clone(),
                output: None,
                exit_code: 1,
                stdout: String::new(),
                stderr: String::new(),
            };
            if let Some(ExecutionUnit {
                unit_id: _,
                language: _,
                code,
            }) = exec_unit
            {
                if let Some(DataObject {
                    object_id: _,
                    format: _,
                    data,
                }) = code
                {
                    match lyric
                        .with_wasm_runtime(move |rt| async move {
                            // parse path from data
                            let data_path = String::from_utf8(data).map_err(|e| {
                                Error::InternalError(format!("Failed to parse data: {:?}", e))
                            })?;
                            let data = tokio::fs::read(data_path.as_str()).await.map_err(|e| {
                                Error::InternalError(format!("Failed to read data: {:?}", e))
                            })?;
                            tracing::info!(
                                "Launching component: {:?} from path: {:?}",
                                task_id,
                                data_path
                            );
                            rt.launch_component(task_id.as_str(), data)
                                .await
                                .map_err(|e| {
                                    Error::InternalError(format!(
                                        "Failed to launch component: {:?}",
                                        e
                                    ))
                                })?;
                            let address = String::from(rt.address());
                            let data_object = DataObject {
                                object_id: "".to_string(),
                                format: DataFormat::Raw as i32,
                                data: address.as_bytes().to_vec(),
                            };
                            Ok(data_object)
                        })
                        .await
                    {
                        Ok(output) => {
                            state_info.output = Some(output);
                            state_info.state = TaskState::TaskSucceeded as i32;
                            state_info.exit_code = 0;
                        }
                        Err(e) => {
                            state_info.state = TaskState::TaskFailed as i32;
                            state_info.stderr = format!("Failed to launch component: {:?}", e);
                        }
                    }
                }
            }
            let _ = tx.send(Ok(TaskStateResult::TaskState(state_info)));
        }
        LangWorkerMessage::StopComponentTask {
            task_id,
            tx,
            worker_id: _,
        } => {
            tracing::debug!("Received StopComponentTask message");
            match lyric
                .with_wasm_runtime(move |rt| async move {
                    rt.remove_component(task_id.as_str()).await.map_err(|e| {
                        Error::InternalError(format!("Failed to stop component task: {:?}", e))
                    })
                })
                .await
            {
                Ok(_) => {
                    tracing::debug!("Component task stopped successfully");
                    let _ = tx.send(Ok(()));
                }
                Err(e) => {
                    let _ = tx.send(Err(Error::InternalError(format!(
                        "Failed to stop component task: {:?}",
                        e
                    ))));
                }
            }
        }
    }
    Ok(())
}

#[tracing::instrument(level = "debug", skip_all, fields(task_id = ?task_id, worker_id=?worker_id))]
fn process_callback_result(
    result: PyObject,
    tx: ResultSender<TaskStateResult, Error>,
    task_id: String,
    worker_id: String,
    start_time: i64,
) -> PyResult<()> {
    Python::with_gil(|py| {
        let callback_result =
            extract_callback_result(py, result, task_id.clone(), worker_id.clone(), start_time);
        let _ = tx.send(callback_result);
        Ok(())
    })
}

fn extract_callback_result(
    py: Python,
    result: PyObject,
    task_id: String,
    worker_id: String,
    start_time: i64,
) -> Result<TaskStateResult, Error> {
    let end_time = Local::now().timestamp_millis();
    let mut state_info = TaskStateInfo {
        task_id,
        state: TaskState::TaskFailed as i32,
        start_time: start_time.clone(),
        end_time: end_time,
        worker_id,
        output: None,
        exit_code: 1,
        stdout: String::new(),
        stderr: String::new(),
    };
    if result.is_none(py) {
        state_info.state = TaskState::TaskSucceeded as i32;
        state_info.exit_code = 0;
        return Ok(TaskStateResult::TaskState(state_info));
    }
    let str_res = if let Ok(value) = result.downcast_bound::<PyString>(py) {
        let s = value.to_cow().map_err(pyerr_to_lyric_err)?;
        // String::from(s.as_ref())
        Err(Error::InternalError(
            "String return type not supported".to_string(),
        ))
    } else if let Ok(value) = result.downcast_bound::<PyDict>(py) {
        Err(Error::InternalError(
            "Dict return type not supported".to_string(),
        ))
    } else if let Ok(value) = result.downcast_bound::<PyList>(py) {
        Err(Error::InternalError(
            "List return type not supported".to_string(),
        ))
    } else if let Ok(mut value) = result.downcast_bound::<PyIterator>(py) {
        Err(Error::InternalError(
            "Iterator return type not supported".to_string(),
        ))
    } else if let Ok(mut task_output) = result.extract::<PyTaskOutputObject>(py) {
        // let output = task_output.get_inner();
        let output = DataObject::from(task_output.data);
        state_info.state = TaskState::TaskSucceeded as i32;
        state_info.exit_code = 0;
        state_info.output = Some(output);
        state_info.stdout = task_output.stdout;
        state_info.stderr = task_output.stderr;
        Ok(TaskStateResult::TaskState(state_info))
    } else if let Ok(mut py_stream) = result.extract::<PyRef<PyUnboundedReceiverStream>>(py) {
        tracing::debug!("Received item from stream from execute result");
        // 创建一个新的 UnboundedReceiverStream
        let (tx, rx) = unbounded_channel();
        let new_stream = UnboundedReceiverStream::new(rx);

        let inner_stream = py_stream.get_inner();
        let new_state_info = state_info.clone();

        tokio::task::spawn_blocking(move || {
            let mut stream = inner_stream.lock().unwrap();
            while let Some(item) = futures::executor::block_on(stream.next()) {
                let mut s = new_state_info.clone();
                let output = DataObject::from(item.data);
                s.output = Some(output);
                s.stdout = item.stdout;
                s.stderr = item.stderr;
                let _ = tx.send(Ok(s));
            }
            // Send stream end signal
            let _ = tx.send(Err(TaskError::StreamStopped));
            tracing::debug!("Stream finished");
        });
        Ok(TaskStateResult::StreamTaskState(new_stream))
    } else {
        Err(Error::InternalError(format!(
            "Unsupported return type from callback, {:?}",
            result
        )))
    };
    str_res
}
