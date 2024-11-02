use futures::stream::StreamExt;
use pyo3::prelude::*;
use pyo3::types::PyIterator;

#[pyfunction]
pub fn process_python_iterator(py: Python, obj: PyObject) -> PyResult<PyObject> {
    let iter = PyIterator::from_bound_object(obj.bind(py))?;
    Ok(iter.to_object(py))
}
