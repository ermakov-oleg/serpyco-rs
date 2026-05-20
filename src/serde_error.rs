//! Internal error type used throughout the serializer/validator.
//!
//! `SerdeError` distinguishes two cases that matter at the FFI boundary:
//!   - `Schema`: a validation/serialization failure. These are recoverable
//!     inside `Union` (the union tries the next variant) and ultimately
//!     surface as `SchemaValidationError` on the Python side.
//!   - `Py`: a raw `PyErr` that must propagate as-is — used for
//!     `BaseException`-only types like `KeyboardInterrupt`/`SystemExit`
//!     so they cannot be silently swallowed by a union branch.
//!
//! `from_user_callback` is the single place that decides which case a
//! Python exception raised from user code falls into.

use pyo3::exceptions::PyException;
use pyo3::prelude::*;
use pyo3::types::PyType;

use crate::errors::{ErrorItem, SchemaValidationError};
use crate::validator::errors::into_path;
use crate::validator::InstancePath;

#[derive(Debug)]
pub(crate) enum SerdeError {
    Schema(SchemaError),
    Py(PyErr),
}

#[derive(Debug)]
pub(crate) struct SchemaError {
    pub(crate) message: String,
    pub(crate) path: String,
    pub(crate) cause: Option<PyErr>,
}

impl SchemaError {
    pub(crate) fn new(message: String, path: &InstancePath) -> Self {
        Self {
            message,
            path: into_path(path),
            cause: None,
        }
    }

    pub(crate) fn with_cause(message: String, path: &InstancePath, cause: PyErr) -> Self {
        Self {
            message,
            path: into_path(path),
            cause: Some(cause),
        }
    }
}

impl From<PyErr> for SerdeError {
    #[inline]
    fn from(err: PyErr) -> Self {
        SerdeError::Py(err)
    }
}

impl From<SchemaError> for SerdeError {
    #[inline]
    fn from(err: SchemaError) -> Self {
        SerdeError::Schema(err)
    }
}

impl SerdeError {
    /// Single conversion point to a Python error — invoked at the FFI boundary.
    pub(crate) fn into_py_err(self) -> PyErr {
        match self {
            SerdeError::Py(err) => err,
            SerdeError::Schema(s) => Python::attach(|py| {
                let errors: Vec<ErrorItem> = vec![ErrorItem::new(s.message, s.path)];
                let py_err = PyErr::from_type(
                    PyType::new::<SchemaValidationError>(py),
                    ("Schema validation failed".to_string(), errors),
                );
                if let Some(cause) = s.cause {
                    py_err.set_cause(py, Some(cause));
                }
                py_err
            }),
        }
    }

    /// Wraps a PyErr raised from a user callback.
    /// Regular `Exception` subclasses become `Schema` with `cause` (discarded inside Union).
    /// `BaseException`-only types (`KeyboardInterrupt`, `SystemExit`) go to `Py` and propagate.
    pub(crate) fn from_user_callback(err: PyErr, path: &InstancePath) -> Self {
        Python::attach(|py| {
            if !err.is_instance_of::<PyException>(py) {
                return SerdeError::Py(err);
            }
            SerdeError::Schema(SchemaError::with_cause(err.to_string(), path, err))
        })
    }
}

pub(crate) type SerdeResult<T> = Result<T, SerdeError>;
