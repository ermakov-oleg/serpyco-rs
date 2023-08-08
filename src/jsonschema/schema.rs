use super::format::{datetime_validator, decimal_validator, time_validator, uuid_validator};
use super::ser;
use crate::errors::{ErrorItem, SchemaValidationError, ToPyErr, ValidationError};
use crate::jsonschema::format::date_validator;
use crate::python::py_str_to_str;
use jsonschema::{Draft, JSONSchema};
use pyo3::types::PyList;
use pyo3::types::PyType;
use pyo3::{AsPyPointer, IntoPy, Py, PyAny, PyErr, PyResult, Python};
use serde_json::Value;

pub(crate) fn compile(schema: &PyAny, pass_through_bytes: bool) -> PyResult<JSONSchema> {
    let schema_str = py_str_to_str(schema.as_ptr())?;
    let serde_schema: Value = serde_json::from_str(schema_str)
        .map_err(|e| ValidationError::new_err(format!("Error while parsing JSON string: {}", e)))?;

    let mut options = JSONSchema::options();
    let schema_options = options
        .with_draft(Draft::Draft202012)
        .with_format("date-time", datetime_validator)
        .with_format("date", date_validator)
        .with_format("time", time_validator)
        .with_format("uuid", uuid_validator)
        .with_format("decimal", decimal_validator)
        .should_validate_formats(true)
        .should_ignore_unknown_formats(false);

    if pass_through_bytes {
        schema_options.with_format("binary", |_| true);
    }

    let compiled = schema_options
        .compile(&serde_schema)
        .map_err(|e| ValidationError::new_err(format!("Invalid json schema: {}", e)))?;

    Ok(compiled)
}

pub(crate) fn validate_python(
    compiled: &JSONSchema,
    pass_through_bytes: bool,
    instance: &PyAny,
) -> PyResult<()> {
    let serde_value = ser::to_value(instance, pass_through_bytes)?;
    validate(instance.py(), compiled, &serde_value)
}

pub(crate) fn validate(py: Python<'_>, compiled: &JSONSchema, instance: &Value) -> PyResult<()> {
    // is valid significantly faster than validate
    if compiled.is_valid(instance) {
        return Ok(());
    }
    if let Err(result) = compiled.validate(instance) {
        let errors = PyList::empty(py);
        for error in result {
            errors.append(into_err_item(py, error)?)?;
        }
        let errors: Py<PyList> = errors.into_py(py);

        let pyerror_type = PyType::new::<SchemaValidationError>(py);
        return Err(PyErr::from_type(
            pyerror_type,
            ("Schema validation failed".to_string(), errors),
        ));
    }
    Ok(())
}

fn into_err_item(
    py: Python<'_>,
    error: jsonschema::ValidationError<'_>,
) -> PyResult<Py<ErrorItem>> {
    let message = error.to_string();
    let schema_path = into_path(error.schema_path);
    let instance_path = into_path(error.instance_path);
    Py::new(py, ErrorItem::new(message, schema_path, instance_path))
}

fn into_path(pointer: jsonschema::paths::JSONPointer) -> String {
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
    path.join("/")
}
