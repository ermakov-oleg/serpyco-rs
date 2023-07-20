use super::encoders::ValidationError;
use pyo3::{
    prelude::*,
    types::{PyList, PyType},
};
use pyo3::{Py, PyErr, PyResult, Python};
use serde_json::Value;

pyo3::create_exception!(serpyco_rs, SchemaValidationError, ValidationError);

pub fn raise_on_error(
    py: Python<'_>,
    compiled: &jsonschema::JSONSchema,
    instance: &Value,
) -> PyResult<()> {
    // is valid significantly faster than validate
    if compiled.is_valid(instance) {
        return Ok(());
    }
    let result = compiled.validate(instance);
    let error = result
        .err()
        .map(|mut errors| errors.next().expect("Iterator should not be empty"));
    error.map_or_else(|| Ok(()), |err| Err(into_py_err(py, err)?))
}

fn into_py_err(py: Python<'_>, error: jsonschema::ValidationError<'_>) -> PyResult<PyErr> {
    let pyerror_type = PyType::new::<SchemaValidationError>(py);
    let message = error.to_string();
    let verbose_message = to_error_message(&error);
    let schema_path = into_path(py, error.schema_path)?;
    let instance_path = into_path(py, error.instance_path)?;
    Ok(PyErr::from_type(
        pyerror_type,
        (message, verbose_message, schema_path, instance_path),
    ))
}

fn to_error_message(error: &jsonschema::ValidationError<'_>) -> String {
    let mut message = error.to_string();
    message.push('\n');
    message.push('\n');
    message.push_str("Failed validating");

    let push_quoted = |m: &mut String, s: &str| {
        m.push('"');
        m.push_str(s);
        m.push('"');
    };

    let push_chunk = |m: &mut String, chunk: &jsonschema::paths::PathChunk| {
        match chunk {
            jsonschema::paths::PathChunk::Property(property) => push_quoted(m, property),
            jsonschema::paths::PathChunk::Index(index) => m.push_str(&index.to_string()),
            jsonschema::paths::PathChunk::Keyword(keyword) => push_quoted(m, keyword),
        };
    };

    if let Some(last) = error.schema_path.last() {
        message.push(' ');
        push_chunk(&mut message, last);
    }
    message.push_str(" in schema");
    let mut chunks = error.schema_path.iter().peekable();
    while let Some(chunk) = chunks.next() {
        // Skip the last element as it is already mentioned in the message
        if chunks.peek().is_none() {
            break;
        }
        message.push('[');
        push_chunk(&mut message, chunk);
        message.push(']');
    }
    message.push('\n');
    message.push('\n');
    message.push_str("On instance");
    for chunk in &error.instance_path {
        message.push('[');
        match chunk {
            jsonschema::paths::PathChunk::Property(property) => push_quoted(&mut message, property),
            jsonschema::paths::PathChunk::Index(index) => message.push_str(&index.to_string()),
            // Keywords are not used for instances
            jsonschema::paths::PathChunk::Keyword(_) => unreachable!("Internal error"),
        };
        message.push(']');
    }
    message.push(':');
    message.push_str("\n    ");
    message.push_str(&error.instance.to_string());
    message
}

fn into_path(py: Python<'_>, pointer: jsonschema::paths::JSONPointer) -> PyResult<Py<PyList>> {
    let path = PyList::empty(py);
    for chunk in pointer {
        match chunk {
            jsonschema::paths::PathChunk::Property(property) => {
                path.append(property.into_string())?;
            }
            jsonschema::paths::PathChunk::Index(index) => path.append(index)?,
            jsonschema::paths::PathChunk::Keyword(keyword) => path.append(keyword)?,
        };
    }
    Ok(path.into_py(py))
}
