use lyric_utils::prelude::Error;
use pyo3::{IntoPy, PyErr, PyObject, Python};

pub(crate) fn pyerr_to_lyric_err(e: PyErr) -> Error {
    Python::with_gil(|py| {
        let e_as_object: PyObject = e.into_py(py);

        match e_as_object.call_method_bound(py, "__str__", (), None) {
            Ok(repr) => match repr.extract::<String>(py) {
                Ok(s) => Error::InternalError(s),
                Err(_e) => Error::InternalError("An unknown error has occurred".to_string()),
            },
            Err(_) => Error::InternalError("Err doesn't have __str__".to_string()),
        }
    })
}
