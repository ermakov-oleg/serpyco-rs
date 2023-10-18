use pyo3::types::{PyList, PyType};
use pyo3::{IntoPy, Py, PyErr, PyResult, Python};

use crate::errors::ErrorItem;
use crate::errors::SchemaValidationError;
use crate::validator::context::PathChunk;
use crate::validator::InstancePath;

pub fn raise_error(error: String, instance_path: &InstancePath) -> PyResult<()> {
    Python::with_gil(|py| {
        let errors = PyList::empty(py);
        errors.append(into_err_item(py, error, instance_path)?)?;
        let errors: Py<PyList> = errors.into_py(py);

        let pyerror_type = PyType::new::<SchemaValidationError>(py);
        Err(PyErr::from_type(
            pyerror_type,
            ("Schema validation failed".to_string(), errors),
        ))
    })
}

fn into_err_item(
    py: Python<'_>,
    error: String,
    instance_path: &InstancePath,
) -> PyResult<Py<ErrorItem>> {
    let message = error.to_string();
    let instance_path = into_path(instance_path);
    Py::new(py, ErrorItem::new(message, instance_path))
}

fn into_path(pointer: &InstancePath) -> String {
    let mut path = vec![];
    for chunk in pointer.to_vec() {
        match chunk {
            PathChunk::Property(property) => {
                path.push(property.to_string());
            }
            PathChunk::Index(index) => path.push(index.to_string()),
            PathChunk::Keyword(keyword) => path.push(keyword.to_string()),
        };
    }
    path.join("/")
}
