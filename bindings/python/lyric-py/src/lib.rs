mod config;
mod env;
mod error;
mod handle;
mod lyric;
mod task;
mod types;

use crate::handle::{PyTaskCallArgs, PyTaskHandle};
use config::{PyConfig, PyDriverConfig, PyWorkerConfig};
use env::{PyDockerEnvironmentConfig, PyEnvironmentConfig, PyLocalEnvironmentConfig};
use lyric::PyLyric;
use pyo3::prelude::*;
use task::{
    PyDataObject, PyExecutionUnit, PyStreamDataObjectIter, PyTaskInfo, PyTaskOutputObject,
    PyTaskStateInfo,
};
use types::from_python_iterator;

/// A Python module implemented in Rust.
#[pymodule]
fn _py_lyric(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyLyric>().unwrap();
    m.add_class::<PyTaskInfo>().unwrap();
    m.add_class::<PyDataObject>().unwrap();
    m.add_class::<PyTaskOutputObject>().unwrap();
    m.add_class::<PyStreamDataObjectIter>().unwrap();
    m.add_class::<PyTaskStateInfo>().unwrap();
    m.add_class::<PyExecutionUnit>().unwrap();
    m.add_class::<PyConfig>().unwrap();
    m.add_class::<PyDriverConfig>().unwrap();
    m.add_class::<PyWorkerConfig>().unwrap();
    m.add_class::<PyEnvironmentConfig>().unwrap();
    m.add_class::<PyLocalEnvironmentConfig>().unwrap();
    m.add_class::<PyDockerEnvironmentConfig>().unwrap();
    m.add_class::<PyTaskCallArgs>().unwrap();
    m.add_class::<PyTaskHandle>().unwrap();
    m.add_function(wrap_pyfunction!(from_python_iterator, m)?);
    Ok(())
}
