use pyo3::exceptions::PyException;
use pyo3::prelude::*;
use pyo3::types::PyType;

use crate::errors::{ErrorItem, SchemaValidationError};
use crate::validator::InstancePath;
use crate::validator::errors::into_path;

#[allow(dead_code)]
#[derive(Debug)]
pub(crate) enum SerdeError {
    Schema(SchemaError),
    Py(PyErr),
}

#[allow(dead_code)]
#[derive(Debug)]
pub(crate) struct SchemaError {
    pub(crate) message: String,
    pub(crate) path: String,
    pub(crate) cause: Option<PyErr>,
}

#[allow(dead_code)]
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

#[allow(dead_code)]
impl SerdeError {
    /// Единственная точка конвертации в Python-ошибку — вызывается на FFI-границе.
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

    /// Конвертер для PyErr из пользовательских callback'ов.
    /// Обычные `Exception` оборачиваются в `Schema` с `cause` (отбрасываются в Union).
    /// `BaseException`-only (`KeyboardInterrupt`, `SystemExit`) уходят в `Py` (всплывают).
    pub(crate) fn from_user_callback(err: PyErr, path: &InstancePath) -> Self {
        Python::attach(|py| {
            if !err.is_instance_of::<PyException>(py) {
                return SerdeError::Py(err);
            }
            SerdeError::Schema(SchemaError::with_cause(err.to_string(), path, err))
        })
    }
}

#[allow(dead_code)]
pub(crate) type SerdeResult<T> = Result<T, SerdeError>;
