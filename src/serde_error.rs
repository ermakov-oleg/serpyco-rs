use pyo3::exceptions::PyException;
use pyo3::prelude::*;
use pyo3::types::PyType;

use crate::errors::{ErrorItem, SchemaValidationError};
use crate::validator::InstancePath;
use crate::validator::errors::into_path;

#[derive(Debug)]
pub enum SerdeError {
    Schema(SchemaError),
    Py(PyErr),
}

#[derive(Debug)]
pub struct SchemaError {
    pub message: String,
    pub path: String,
    pub cause: Option<PyErr>,
}

impl SchemaError {
    pub fn new(message: String, path: &InstancePath) -> Self {
        Self {
            message,
            path: into_path(path),
            cause: None,
        }
    }

    pub fn with_cause(message: String, path: &InstancePath, cause: PyErr) -> Self {
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
    /// Единственная точка конвертации в Python-ошибку — вызывается на FFI-границе.
    pub fn into_py_err(self) -> PyErr {
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
    pub fn from_user_callback(err: PyErr, path: &InstancePath) -> Self {
        Python::attach(|py| {
            if !err.is_instance_of::<PyException>(py) {
                return SerdeError::Py(err);
            }
            SerdeError::Schema(SchemaError::with_cause(err.to_string(), path, err))
        })
    }
}

pub type SerdeResult<T> = Result<T, SerdeError>;

#[cfg(test)]
mod tests {
    use super::*;

    fn init_python() {
        pyo3::prepare_freethreaded_python();
    }

    #[test]
    fn from_pyerr_makes_py_variant() {
        init_python();
        Python::attach(|_py| {
            let pyerr = PyErr::new::<pyo3::exceptions::PyRuntimeError, _>("boom");
            let err: SerdeError = pyerr.into();
            assert!(matches!(err, SerdeError::Py(_)));
        });
    }

    #[test]
    fn schema_new_materializes_empty_path() {
        let path = InstancePath::new();
        let err = SchemaError::new("nope".to_string(), &path);
        assert_eq!(err.message, "nope");
        assert_eq!(err.path, "");
        assert!(err.cause.is_none());
    }

    #[test]
    fn from_user_callback_wraps_value_error_as_schema() {
        init_python();
        Python::attach(|_py| {
            let pyerr = PyErr::new::<pyo3::exceptions::PyValueError, _>("bad");
            let path = InstancePath::new();
            let err = SerdeError::from_user_callback(pyerr, &path);
            match err {
                SerdeError::Schema(s) => {
                    assert!(s.cause.is_some());
                    assert!(s.message.contains("bad"));
                }
                _ => panic!("expected Schema"),
            }
        });
    }

    #[test]
    fn from_user_callback_keeps_keyboard_interrupt_as_py() {
        init_python();
        Python::attach(|_py| {
            let pyerr = PyErr::new::<pyo3::exceptions::PyKeyboardInterrupt, _>("");
            let path = InstancePath::new();
            let err = SerdeError::from_user_callback(pyerr, &path);
            assert!(matches!(err, SerdeError::Py(_)));
        });
    }
}
