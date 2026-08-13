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

use std::sync::Arc;

use pyo3::exceptions::PyException;
use pyo3::prelude::*;
use pyo3::types::{PyString, PyType};

use crate::errors::{ErrorItem, SchemaValidationError};
use crate::python::fmt_py;
use crate::validator::errors::into_path;
use crate::validator::InstancePath;

#[derive(Debug)]
pub(crate) enum SerdeError {
    Schema(SchemaError),
    Py(PyErr),
}

/// What a value was expected to be, kept unrendered until the message is built.
#[derive(Debug)]
pub(crate) enum Expected {
    Static(&'static str),
    Shared(Arc<str>),
    Cls(Py<PyType>),
}

impl From<&'static str> for Expected {
    fn from(v: &'static str) -> Self {
        Expected::Static(v)
    }
}

impl From<Arc<str>> for Expected {
    fn from(v: Arc<str>) -> Self {
        Expected::Shared(v)
    }
}

impl From<Py<PyType>> for Expected {
    fn from(v: Py<PyType>) -> Self {
        Expected::Cls(v)
    }
}

impl Expected {
    fn render(&self, py: Python<'_>) -> String {
        match self {
            Expected::Static(s) => (*s).to_string(),
            Expected::Shared(s) => s.to_string(),
            Expected::Cls(cls) => match cls.bind(py).name() {
                Ok(name) => name.to_string(),
                Err(_) => "<unknown>".to_string(),
            },
        }
    }
}

/// Deferred error text: a union discards every message but the one from the member
/// it finally accepts, and rendering costs a Python `str()` call — so parts are
/// captured here and formatted only if the error reaches the FFI boundary.
#[derive(Debug)]
pub(crate) enum Message {
    Text(String),
    /// Load-side mismatch; the value renders via `fmt_py` (quoted only if a str).
    NotOfType {
        value: Py<PyAny>,
        expected: Expected,
    },
    /// Dump-side mismatch; the value is always quoted (matches the old format).
    NotOfTypeDump {
        value: Py<PyAny>,
        expected: Expected,
    },
    NotOneOf {
        value: Py<PyAny>,
        items: Arc<str>,
    },
    /// Load-side absent field. The name is already a `Py<PyString>` on the
    /// encoder, so capturing it is a refcount bump instead of a `format!`.
    MissingProperty {
        name: Py<PyString>,
    },
    /// Dump-side absent `TypedDict` key — the untagged-union probe that hits
    /// this on every non-matching member, hence the same deferral.
    MissingDictKey {
        name: Py<PyString>,
    },
}

impl Message {
    fn render(self, py: Python<'_>) -> String {
        match self {
            Message::Text(text) => text,
            Message::NotOfType { value, expected } => format!(
                r#"{} is not of type "{}""#,
                fmt_py(value.bind(py)),
                expected.render(py)
            ),
            Message::NotOfTypeDump { value, expected } => format!(
                r#""{}" is not of type "{}""#,
                value.bind(py),
                expected.render(py)
            ),
            Message::NotOneOf { value, items } => {
                format!("{} is not one of {}", fmt_py(value.bind(py)), items)
            }
            Message::MissingProperty { name } => {
                format!(r#""{}" is a required property"#, name.bind(py))
            }
            Message::MissingDictKey { name } => format!(
                "data dictionary is missing required parameter {}",
                name.bind(py)
            ),
        }
    }
}

#[derive(Debug)]
pub(crate) struct SchemaError {
    pub(crate) message: Message,
    /// `None` for the (common) root path — keeps `SerdeError` small enough that
    /// every `SerdeResult` stays cheap to return.
    pub(crate) path: Option<Box<str>>,
    pub(crate) cause: Option<PyErr>,
}

/// Root paths are empty; store them as `None` instead of an empty `String`.
fn path_of(path: &InstancePath) -> Option<Box<str>> {
    let rendered = into_path(path);
    (!rendered.is_empty()).then(|| rendered.into_boxed_str())
}

impl SchemaError {
    pub(crate) fn new(message: String, path: &InstancePath) -> Self {
        Self {
            message: Message::Text(message),
            path: path_of(path),
            cause: None,
        }
    }

    /// Build from an unrendered [`Message`] — the constructor used on paths a
    /// union probes, where the error is usually thrown away.
    pub(crate) fn deferred(message: Message, path: &InstancePath) -> Self {
        Self {
            message,
            path: path_of(path),
            cause: None,
        }
    }

    /// [`deferred`](Self::deferred) that also chains `cause`: a union discards it
    /// with the error, but anything reaching Python carries the original.
    pub(crate) fn deferred_with_cause(message: Message, path: &InstancePath, cause: PyErr) -> Self {
        Self {
            message,
            path: path_of(path),
            cause: Some(cause),
        }
    }

    pub(crate) fn with_cause(message: String, path: &InstancePath, cause: PyErr) -> Self {
        Self {
            message: Message::Text(message),
            path: path_of(path),
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
                let path = s.path.map(String::from).unwrap_or_default();
                let errors: Vec<ErrorItem> = vec![ErrorItem::new(s.message.render(py), path)];
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
