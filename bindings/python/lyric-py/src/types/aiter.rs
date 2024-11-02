use crate::task::PyTaskOutputObject;
use futures::stream::Stream;
use pyo3::prelude::*;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll};
use tokio::sync::mpsc::{unbounded_channel, UnboundedReceiver};

pub struct UnboundedReceiverStream<T> {
    inner: UnboundedReceiver<T>,
}

impl<T> UnboundedReceiverStream<T> {
    fn new(inner: UnboundedReceiver<T>) -> Self {
        Self { inner }
    }
}

impl<T> Stream for UnboundedReceiverStream<T> {
    type Item = T;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        self.inner.poll_recv(cx)
    }
}

struct ThreadSafePyIterator {
    iter: Arc<Mutex<PyObject>>,
}

impl ThreadSafePyIterator {
    fn new(iter: PyObject) -> Self {
        Self {
            iter: Arc::new(Mutex::new(iter)),
        }
    }

    fn next(&self) -> PyResult<Option<PyTaskOutputObject>> {
        Python::with_gil(|py| {
            let iter = self.iter.lock().unwrap();
            let next_item = iter.call_method1(py, "__next__", ())?;
            if next_item.is_none(py) {
                Ok(None)
            } else {
                next_item.extract::<PyTaskOutputObject>(py).map(Some)
            }
        })
    }
}

unsafe impl Send for ThreadSafePyIterator {}

#[pyclass]
pub struct PyUnboundedReceiverStream {
    pub inner: Arc<Mutex<UnboundedReceiverStream<PyTaskOutputObject>>>,
}

impl PyUnboundedReceiverStream {
    pub fn get_inner(&self) -> Arc<Mutex<UnboundedReceiverStream<PyTaskOutputObject>>> {
        self.inner.clone()
    }
}

#[pymethods]
impl PyUnboundedReceiverStream {
    fn __iter__(slf: PyRef<'_, Self>) -> PyRef<'_, Self> {
        slf
    }
}

#[pyfunction]
pub fn from_python_iterator(
    py: Python<'_>,
    iterator: PyObject,
) -> PyResult<Py<PyUnboundedReceiverStream>> {
    let (tx, rx) = unbounded_channel();
    let stream = UnboundedReceiverStream::new(rx);

    let safe_iter = ThreadSafePyIterator::new(iterator);

    std::thread::spawn(move || {
        while let Ok(Some(item)) = safe_iter.next() {
            tracing::debug!("from_python_iterator Sending item: {:?}", item);
            let _ = tx.send(item);
        }
    });

    let py_stream = PyUnboundedReceiverStream {
        inner: Arc::new(Mutex::new(stream)),
    };
    Py::new(py, py_stream)
}
