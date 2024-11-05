use async_trait::async_trait;
use futures::TryFutureExt;
use std::future::Future;
use std::sync::Arc;
// use pyo3::{pyclass, pymethods, PyErr, PyObject, PyRef, PyRefMut, PyResult};
use crate::resource::PyTaskResourceConfig;
use crate::task::{PyDataObject, PyTaskStateInfo};
use lyric::task_ext::{ClientType, MsgpackDeserializeExt, MsgpackSerializeExt, TaskHandle};
use lyric::{Lyric, TaskDescription, TokioRuntime};
use lyric_rpc::task::{DataFormat, DataObject};
use lyric_utils::err::Error;
use lyric_wasm_runtime::Handler;
use pyo3::prelude::*;
use tokio::sync::Mutex;
use wrpc_transport::Invoke;

#[async_trait]
trait TaskCaller {
    async fn call(&self, args: Option<PyTaskCallArgs>) -> PyResult<PyDataObject>;
}

#[pyclass]
#[derive(Clone)]
pub struct PyTaskCallArgs {
    pub(crate) data: Option<PyDataObject>,
}

#[pymethods]
impl PyTaskCallArgs {
    #[new]
    #[pyo3(signature = (data = None))]
    fn new(data: Option<PyDataObject>) -> Self {
        Self { data }
    }
}

#[pyclass]
#[derive(Clone)]
pub struct PyTaskHandle {
    pub(crate) inner: Arc<Mutex<TaskHandle<String>>>,
}

#[pymethods]
impl PyTaskHandle {
    #[pyo3(name = "run", signature = (args, resources = None))]
    async fn run(
        &self,
        args: PyTaskCallArgs,
        resources: Option<PyTaskResourceConfig>,
    ) -> PyResult<PyDataObject> {
        use lyric_wasm_runtime::capability::wrpc::lyric::task::{binary_task, types};
        let req_data = args
            .data
            .ok_or(PyErr::new::<pyo3::exceptions::PyTypeError, _>(
                "data is required",
            ))?;
        let req = types::BinaryRequest {
            resources: resources.map(|r| r.into_rpc()),
            protocol: 1_u32,
            data: req_data.data.into(),
        };
        let th = self.inner.lock().await;
        let rt = th.runtime.clone();
        let handler = th.copy_handler();
        drop(th);

        let res = rt
            .runtime
            .spawn(async move { binary_task::run(&handler, (), &req).await })
            .await
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(format!("{:?}", e)))?;

        let bytes = if let Ok(Ok(types::BinaryResponse { protocol, data })) = res {
            data.into()
        } else {
            "Run task wasm worker failed, no response found"
                .as_bytes()
                .to_vec()
        };
        let response_data = DataObject {
            object_id: "".to_string(),
            format: DataFormat::Pickle as i32,
            data: bytes,
        };
        Ok(PyDataObject::from(response_data))
    }

    #[pyo3(name = "exec", signature = (lang, code, decode = true, resources = None))]
    async fn exec(
        &self,
        lang: String,
        code: String,
        decode: bool,
        resources: Option<PyTaskResourceConfig>,
    ) -> PyResult<PyDataObject> {
        use lyric_wasm_runtime::capability::wrpc::lyric::task::{interpreter_task, types};
        let req = types::InterpreterRequest {
            resources: resources.map(|r| r.into_rpc()),
            protocol: 1_u32,
            lang,
            code,
        };
        self.do_exec(move |handler| async move {
            let res = interpreter_task::run(&handler, (), &req).await;
            let res: PyResult<PyDataObject> = match res {
                Ok(Ok(types::InterpreterResponse { protocol, data })) => {
                    let data = handle_exec_result(Ok(data), decode)?;
                    Ok(data)
                }
                Ok(Err(e)) => Err(PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(format!(
                    "{:?}",
                    e
                ))),
                Err(e) => Err(PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(format!(
                    "{:?}",
                    e
                ))),
            };
            res
        })
        .await
    }

    #[pyo3(name = "exec1", signature = (lang, code, call_name, input, encode, decode = true, resources = None))]
    async fn exec1(
        &self,
        lang: String,
        code: String,
        call_name: String,
        input: Vec<u8>,
        encode: bool,
        decode: bool,
        resources: Option<PyTaskResourceConfig>,
    ) -> PyResult<Vec<PyDataObject>> {
        use lyric_wasm_runtime::capability::wrpc::lyric::task::{interpreter_task, types};
        let req = types::InterpreterRequest {
            resources: resources.map(|r| r.into_rpc()),
            protocol: 1_u32,
            lang,
            code,
        };
        let input = if encode {
            input.serialize().map_err(|e| {
                PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(format!("{:?}", e))
            })?
        } else {
            input
        };
        self.do_exec(move |handler| async move {
            let input = input.into();
            let res = interpreter_task::run1(&handler, (), &req, &call_name, &input).await;
            match res {
                Ok(Ok(types::InterpreterOutputResponse {
                    protocol,
                    data,
                    output,
                })) => {
                    let data = handle_exec_result(Ok(data), decode)?;
                    let output = handle_exec_result(Ok(output), decode)?;
                    // Ok((data, output))
                    Ok(vec![data, output])
                }
                Ok(Err(e)) => Err(PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(format!(
                    "{:?}",
                    e
                ))),
                Err(e) => Err(PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(format!(
                    "{:?}",
                    e
                ))),
            }
        })
        .await
    }

    async fn stop(&self) -> PyResult<()> {
        self.inner
            .lock()
            .await
            .stop()
            .await
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(format!("{:?}", e)))
    }
}

impl PyTaskHandle {
    async fn do_exec<F, U, T>(&self, f: F) -> PyResult<T>
    where
        F: FnOnce(Handler<ClientType<String>>) -> U + Send + 'static,
        U: Future<Output = PyResult<T>> + Send + 'static,
        T: Send + 'static,
    {
        let th = self.inner.lock().await;
        let rt = th.runtime.clone();
        let handler = th.copy_handler();
        drop(th);
        rt.runtime
            .spawn(async move { f(handler).await })
            .await
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(format!("{:?}", e)))?
    }
}

fn handle_exec_result<T: Into<Vec<u8>>>(
    bytes: PyResult<T>,
    decode: bool,
) -> PyResult<PyDataObject> {
    let handled_bytes = match (bytes, decode) {
        (Ok(bytes), true) => bytes
            .deserialize()
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(format!("{:?}", e))),
        (Ok(bytes), false) => Ok(bytes.into()),
        (Err(e), _) => Err(e),
    };
    let response_data = DataObject {
        object_id: "".to_string(),
        format: DataFormat::Raw as i32,
        data: handled_bytes?,
    };
    Ok(PyDataObject::from(response_data))
}
