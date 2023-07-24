use pyo3::types::PyString;
use pyo3::types::PyType;
use pyo3::{Py, PyErr, PyResult, Python};
use serde_json::Value;

use super::encoders::ValidationError;

pyo3::create_exception!(serpyco_rs, InnerSchemaValidationError, ValidationError);
pyo3::create_exception!(serpyco_rs, InnerErrorItem, ValidationError);

pub fn raise_on_error(
    py: Python<'_>,
    compiled: &jsonschema::JSONSchema,
    instance: &Value,
) -> PyResult<()> {
    // is valid significantly faster than validate
    if compiled.is_valid(instance) {
        return Ok(());
    }
    if let Err(result) = compiled.validate(instance) {
        let mut errors = vec![];
        for error in result {
            errors.push(into_py_err(py, error)?);
        }
        return Err(InnerSchemaValidationError::new_err(errors));
    }
    Ok(())
}

fn into_py_err(py: Python<'_>, error: jsonschema::ValidationError<'_>) -> PyResult<PyErr> {
    let pyerror_type = PyType::new::<InnerErrorItem>(py);
    let message = error.to_string();
    let schema_path = into_path(py, error.schema_path)?;
    let instance_path = into_path(py, error.instance_path)?;
    Ok(PyErr::from_type(
        pyerror_type,
        (message, schema_path, instance_path),
    ))
}

fn into_path(py: Python<'_>, pointer: jsonschema::paths::JSONPointer) -> PyResult<Py<PyString>> {
    let mut path = vec![];
    for chunk in pointer {
        match chunk {
            jsonschema::paths::PathChunk::Property(property) => {
                path.push(property.into_string());
            }
            jsonschema::paths::PathChunk::Index(index) => path.push(index.to_string()),
            jsonschema::paths::PathChunk::Keyword(keyword) => path.push(keyword.to_string()),
        };
    }
    let path = path.join("/");
    Ok(PyString::new(py, &path).into())
}
