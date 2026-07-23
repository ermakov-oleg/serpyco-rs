use pyo3::prelude::*;
use pyo3::types::{PyBool, PyDict, PyFloat, PyInt, PyList, PyString, PyTuple};
use pyo3::IntoPyObjectExt;

use crate::errors::{ToPyErr, ValidationError};
use crate::format::{Kind, Parser, Writer};
use crate::serde_error::{SerdeError, SerdeResult};
use crate::validator::Context;

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
            parser.take_null()?;
            Ok(py.None().into_bound(py))
        }
        Kind::Bool => Ok(PyBool::new(py, parser.take_bool()?).to_owned().into_any()),
        Kind::Num => {
            // Integer when there is no dot/exponent — mirrors json.loads.
            let raw = parser.take_number_str()?;
            if raw.bytes().all(|b| b.is_ascii_digit() || b == b'-') {
                match raw.parse::<i64>() {
                    Ok(v) => Ok(v.into_bound_py_any(py)?),
                    Err(_) => {
                        let big: num_bigint::BigInt = raw.parse().map_err(|_| {
                            SerdeError::Py(ValidationError::new_err(format!(
                                "invalid number: {raw}"
                            )))
                        })?;
                        Ok(big.into_bound_py_any(py)?)
                    }
                }
            } else {
                let v: f64 = raw.parse().map_err(|_| {
                    SerdeError::Py(ValidationError::new_err(format!("invalid number: {raw}")))
                })?;
                Ok(PyFloat::new(py, v).into_any())
            }
        }
        Kind::Str => Ok(PyString::new(py, parser.take_str()?).into_any()),
        Kind::Array => {
            let mut items: Vec<Bound<'py, PyAny>> = Vec::new();
            if parser.enter_array()? {
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
            let dict = PyDict::new(py);
            // Keys borrow from the parser buffer, so copy each one out before
            // the next `parse_any(...)` call can move the cursor and invalidate it.
            let mut key = parser.enter_map()?.map(str::to_owned);
            while let Some(k) = key {
                let py_key = PyString::new(py, &k);
                let value = parse_any(py, parser, ctx)?;
                dict.set_item(py_key, value)?;
                key = parser.next_key()?.map(str::to_owned);
            }
            Ok(dict.into_any())
        }
    }
}

/// Arbitrary Python tree -> writer events. Exact-type sniffing like orjson.
/// Non-string dict keys are written as str(key) (OPT_NON_STR_KEYS behavior).
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
        match v.extract::<i64>() {
            Ok(v) => writer.write_i64(v),
            Err(_) => writer.write_big_int(v.str()?.to_str()?),
        }
        return Ok(());
    }
    if let Ok(v) = value.cast::<PyFloat>() {
        writer
            .write_f64(v.value())
            .map_err(|msg| SerdeError::Py(ValidationError::new_err(msg)))?;
        return Ok(());
    }
    if let Ok(v) = value.cast::<PyString>() {
        writer.write_str(v.to_str()?);
        return Ok(());
    }
    if let Ok(list) = value.cast::<PyList>() {
        writer.begin_array();
        for item in list.iter() {
            writer.array_item();
            write_any(&item, writer, ctx)?;
        }
        writer.end_array();
        return Ok(());
    }
    if let Ok(tup) = value.cast::<PyTuple>() {
        writer.begin_array();
        for item in tup.iter() {
            writer.array_item();
            write_any(&item, writer, ctx)?;
        }
        writer.end_array();
        return Ok(());
    }
    if let Ok(dict) = value.cast::<PyDict>() {
        writer.begin_map();
        for (k, v) in dict.iter() {
            match k.cast::<PyString>() {
                Ok(s) => writer.map_key(s.to_str()?),
                Err(_) => writer.map_key(k.str()?.to_str()?),
            }
            write_any(&v, writer, ctx)?;
        }
        writer.end_map();
        return Ok(());
    }
    Err(SerdeError::Py(ValidationError::new_err(format!(
        "value of type '{}' is not serializable to this format",
        value.get_type().name()?
    ))))
}
