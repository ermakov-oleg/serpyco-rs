use pyo3::prelude::*;
use pyo3::types::{PyBool, PyBytes, PyDict, PyFloat, PyInt, PyList, PyString, PyTuple};
use pyo3::IntoPyObjectExt;

use crate::errors::{ToPyErr, ValidationError};
use crate::format::{Kind, ParsedInt, ParsedNumber, Parser, Writer};
use crate::python::{create_py_dict_known_size, create_py_string};
use crate::serde_error::{SchemaError, SerdeError, SerdeResult};
use crate::validator::{Context, InstancePath};

/// Schema type-mismatch straight from the stream. The rendered value differs from
/// the dict path (raw JSON vs Python repr); `expected` and instance_path match.
pub(crate) fn wrong_type_err(expected: &str, raw: &str, path: &InstancePath) -> SerdeError {
    SchemaError::new(format!(r#"{raw} is not of type "{expected}""#), path).into()
}

/// Same shape as the dict-path enum error (`… is not one of <items>`).
pub(crate) fn wrong_enum_err(items: &str, raw: &str, path: &InstancePath) -> SerdeError {
    SchemaError::new(format!("{raw} is not one of {items}"), path).into()
}

/// [`wrong_type_err`] for the value at the cursor. A parser/decode error while
/// taking that value takes priority over the schema mismatch it would otherwise render.
#[inline]
pub(crate) fn wrong_type_at_cursor(
    parser: &mut Parser<'_>,
    expected: &str,
    path: &InstancePath,
) -> SerdeError {
    match parser.take_value_repr() {
        Ok(raw) => wrong_type_err(expected, &raw, path),
        Err(e) => e,
    }
}

/// Enum-variant of [`wrong_type_at_cursor`] (`… is not one of <items>`).
#[inline]
pub(crate) fn wrong_enum_at_cursor(
    parser: &mut Parser<'_>,
    items: &str,
    path: &InstancePath,
) -> SerdeError {
    match parser.take_value_repr() {
        Ok(raw) => wrong_enum_err(items, &raw, path),
        Err(e) => e,
    }
}

/// `ValidationError("invalid number: …")` — the shared fallback for number text
/// that fails to parse (only reachable on genuinely malformed input).
#[inline]
pub(crate) fn invalid_number_err(raw: &str) -> SerdeError {
    SerdeError::Py(ValidationError::new_err(format!("invalid number: {raw}")))
}

/// Stream a Python `int` to the writer: `i64` fast path, decimal-string fallback beyond i64.
#[inline(always)]
pub(crate) fn write_py_int(writer: &mut Writer, v: &Bound<'_, PyInt>) -> SerdeResult<()> {
    match v.extract::<i64>() {
        Ok(i) => writer.write_i64(i),
        Err(_) => writer
            .write_big_int(v.str()?.to_str()?)
            .map_err(|msg| SerdeError::Py(ValidationError::new_err(msg)))?,
    }
    Ok(())
}

/// Stream a Python `float` to the writer, mapping the format's "cannot represent"
/// signal (JSON: NaN/Infinity) to a `ValidationError`.
#[inline(always)]
pub(crate) fn write_py_float(writer: &mut Writer, v: &Bound<'_, PyFloat>) -> SerdeResult<()> {
    writer
        .write_f64(v.value())
        .map_err(|msg| SerdeError::Py(ValidationError::new_err(msg)))
}

/// Parser events -> plain Python objects (dict/list/str/int/float/bool/None).
/// The point where the schema ends: Any fields, CustomType.load input, dict_flatten.
pub(crate) fn parse_any<'py>(
    py: Python<'py>,
    parser: &mut Parser<'_>,
    ctx: &Context,
) -> SerdeResult<Bound<'py, PyAny>> {
    let _guard = ctx.enter_depth()?;
    match parser.peek()? {
        Kind::Null => {
            parser.take_null_known()?;
            Ok(py.None().into_bound(py))
        }
        Kind::Bool => Ok(PyBool::new(py, parser.take_bool_known()?)
            .to_owned()
            .into_any()),
        Kind::Num => match parser.take_number_known()? {
            ParsedNumber::Int(ParsedInt::I64(v)) => Ok(v.into_bound_py_any(py)?),
            ParsedNumber::Int(ParsedInt::Big(v)) => Ok(v.into_bound_py_any(py)?),
            ParsedNumber::F64(v) => Ok(PyFloat::new(py, v).into_any()),
        },
        Kind::Str => Ok(parser.take_pystring_known(py)?.into_any()),
        Kind::Bytes => Ok(PyBytes::new(py, parser.take_bytes_known()?).into_any()),
        Kind::Array => {
            let mut items: Vec<Bound<'py, PyAny>> = Vec::new();
            if parser.enter_array_known()? {
                items.reserve(parser.container_len_hint().unwrap_or(8));
                loop {
                    items.push(parse_any(py, parser, ctx)?);
                    if !parser.next_array_item()? {
                        break;
                    }
                }
            }
            Ok(PyList::new(py, items)?.into_any())
        }
        Kind::Map => {
            // Materializing the key ends its borrow of the parser buffer, freeing
            // the parser for the recursive value parse.
            let (mut key, len_hint) = parser.enter_map_known_sized()?;
            let dict = match len_hint {
                Some(len) => create_py_dict_known_size(py, len)?,
                None => PyDict::new(py),
            };
            while let Some(k) = key {
                let py_key = create_py_string(py, k)?;
                let value = parse_any(py, parser, ctx)?;
                dict.set_item(py_key, value)?;
                key = parser.next_key()?;
            }
            Ok(dict.into_any())
        }
    }
}

/// Arbitrary Python tree -> writer events. isinstance-based type sniffing
/// (accepts subclasses). Non-string dict keys are stringified via str(key).
pub(crate) fn write_any(
    value: &Bound<'_, PyAny>,
    writer: &mut Writer,
    ctx: &Context,
) -> SerdeResult<()> {
    let _guard = ctx.enter_depth()?;
    if value.is_none() {
        writer.write_null();
        return Ok(());
    }
    // Order matters: bool is a subtype of int in Python.
    if let Ok(v) = value.cast::<PyBool>() {
        writer.write_bool(v.is_true());
        return Ok(());
    }
    if let Ok(v) = value.cast::<PyInt>() {
        write_py_int(writer, v)?;
        return Ok(());
    }
    if let Ok(v) = value.cast::<PyFloat>() {
        write_py_float(writer, v)?;
        return Ok(());
    }
    if let Ok(v) = value.cast::<PyString>() {
        writer.write_str(v.to_str()?);
        return Ok(());
    }
    if let Ok(v) = value.cast::<PyBytes>() {
        writer
            .write_bytes(v.as_bytes())
            .map_err(|msg| SerdeError::Py(ValidationError::new_err(msg)))?;
        return Ok(());
    }
    if let Ok(list) = value.cast::<PyList>() {
        writer.begin_array(Some(list.len()));
        for item in list.iter() {
            write_any(&item, writer, ctx)?;
            writer.item_end();
        }
        writer.end_array();
        return Ok(());
    }
    if let Ok(tup) = value.cast::<PyTuple>() {
        writer.begin_array(Some(tup.len()));
        for item in tup.iter() {
            write_any(&item, writer, ctx)?;
            writer.item_end();
        }
        writer.end_array();
        return Ok(());
    }
    if let Ok(dict) = value.cast::<PyDict>() {
        writer.begin_map(Some(dict.len()));
        for (k, v) in dict.iter() {
            match k.cast::<PyString>() {
                Ok(s) => writer.map_key(s.to_str()?),
                Err(_) => writer.map_key(k.str()?.to_str()?),
            }
            write_any(&v, writer, ctx)?;
            writer.item_end();
        }
        writer.end_map();
        return Ok(());
    }
    Err(SerdeError::Py(ValidationError::new_err(format!(
        "value of type '{}' is not serializable to this format",
        value.get_type().name()?
    ))))
}
