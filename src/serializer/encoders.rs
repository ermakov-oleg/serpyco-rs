use std::collections::HashMap;

use rustc_hash::FxHashMap;
use smallvec::{smallvec, SmallVec};
use std::fmt;
use std::fmt::Debug;
use std::sync::{Arc, OnceLock};

use dyn_clone::{clone_trait_object, DynClone};
use nohash_hasher::IntMap;
use num_bigint::BigInt;
use pyo3::exceptions::{PyAttributeError, PyRuntimeError};
use pyo3::types::{
    PyBool, PyBytes, PyDate, PyDateTime, PyDict, PyFloat, PyInt, PyList, PySequence, PySet,
    PyString, PyTime, PyType,
};
use pyo3::{intern, Bound, Py, PyAny, PyResult};
use pyo3::{prelude::*, IntoPyObjectExt};
use uuid::Uuid;

use crate::errors::{ToPyErr, ValidationError};
use crate::format::bridge::{parse_any, write_any, wrong_enum_err, wrong_type_err};
use crate::format::json::parser::ParsedInt;
use crate::format::{Kind, Parser, Writer};
use crate::python::{
    create_instance, create_py_dict_known_size, create_py_list, create_py_tuple, dump_date,
    dump_datetime, dump_time, generic_set_attr, parse_date, parse_datetime, parse_time,
    py_dict_set_item, py_list_get_item, py_list_set_item, py_tuple_set_item, set_attr_unchecked,
};
use crate::python::{DecimalTypeInfo, FloatTypeInfo, IntegerTypeInfo, StringTypeInfo};
use crate::serde_error::{SerdeError, SerdeResult};
use crate::validator::validators::{
    check_bounds, check_length, check_sequence_bounds, check_sequence_size, invalid_enum_item,
    invalid_type, invalid_type_dump, invalid_type_dump_err, invalid_type_err,
    missing_required_property, no_encoder_for_discriminator, str_as_bool,
};
use crate::validator::{Context, InstancePath};

pub type TEncoder = dyn Encoder + Send + Sync;

pub(crate) trait Encoder: DynClone + Debug {
    fn dump<'a>(&self, value: &Bound<'a, PyAny>, ctx: &Context) -> SerdeResult<Bound<'a, PyAny>>;
    fn load<'a>(
        &self,
        value: &Bound<'a, PyAny>,
        instance_path: &InstancePath,
        ctx: &Context,
    ) -> SerdeResult<Bound<'a, PyAny>>;

    /// Streaming serialization to a format. Default goes through the generic
    /// bridge: identical semantics to dump(); direct impls are an optimization.
    fn dump_format(
        &self,
        value: &Bound<'_, PyAny>,
        writer: &mut Writer,
        ctx: &Context,
    ) -> SerdeResult<()> {
        let dumped = self.dump(value, ctx)?;
        write_any(&dumped, writer, ctx)
    }

    /// Streaming deserialization from a format. Default goes through the bridge.
    fn load_format<'py>(
        &self,
        py: Python<'py>,
        parser: &mut Parser<'_>,
        instance_path: &InstancePath,
        ctx: &Context,
    ) -> SerdeResult<Bound<'py, PyAny>> {
        let value = parse_any(py, parser, ctx)?;
        self.load(&value, instance_path, ctx)
    }

    fn as_container_encoder(&self) -> Option<&dyn ContainerEncoder> {
        None
    }
    fn is_sequence(&self) -> bool {
        false
    }
}

pub struct EncoderField<'a> {
    pub(crate) name: &'a Py<PyString>,
    pub(crate) is_sequence: bool,
}

pub enum QueryFields<'a> {
    Object(Vec<EncoderField<'a>>),
    Dict(bool), // is_sequence
}

pub trait ContainerEncoder: Encoder {
    fn get_fields(&self) -> QueryFields<'_>;
}

clone_trait_object!(Encoder);

#[derive(Debug, Clone)]
pub struct NoopEncoder;

impl Encoder for NoopEncoder {
    #[inline]
    fn dump<'a>(&self, value: &Bound<'a, PyAny>, _ctx: &Context) -> SerdeResult<Bound<'a, PyAny>> {
        Ok(value.clone())
    }

    #[inline]
    fn load<'a>(
        &self,
        value: &Bound<'a, PyAny>,
        _instance_path: &InstancePath,
        _ctx: &Context,
    ) -> SerdeResult<Bound<'a, PyAny>> {
        Ok(value.clone())
    }
}

#[derive(Debug, Clone)]
pub struct NoneEncoder;

impl Encoder for NoneEncoder {
    #[inline]
    fn dump<'a>(&self, value: &Bound<'a, PyAny>, _ctx: &Context) -> SerdeResult<Bound<'a, PyAny>> {
        Ok(value.clone())
    }

    #[inline]
    fn load<'a>(
        &self,
        value: &Bound<'a, PyAny>,
        instance_path: &InstancePath,
        _ctx: &Context,
    ) -> SerdeResult<Bound<'a, PyAny>> {
        if value.is_none() {
            return Ok(value.clone());
        }
        invalid_type!("None", value, instance_path)
    }

    #[inline]
    fn dump_format(
        &self,
        value: &Bound<'_, PyAny>,
        writer: &mut Writer,
        _ctx: &Context,
    ) -> SerdeResult<()> {
        if value.is_none() {
            writer.write_null();
            return Ok(());
        }
        invalid_type_dump!("None", value)
    }

    #[inline]
    fn load_format<'py>(
        &self,
        py: Python<'py>,
        parser: &mut Parser<'_>,
        instance_path: &InstancePath,
        _ctx: &Context,
    ) -> SerdeResult<Bound<'py, PyAny>> {
        if parser.peek()? == Kind::Null {
            parser.take_null_known()?;
            return Ok(py.None().into_bound(py));
        }
        let raw = parser.take_raw_value()?;
        let raw = String::from_utf8_lossy(raw);
        Err(wrong_type_err("None", &raw, instance_path))
    }
}

#[derive(Debug, Clone)]
pub struct NeverEncoder;

impl Encoder for NeverEncoder {
    #[inline]
    fn dump<'a>(&self, value: &Bound<'a, PyAny>, _ctx: &Context) -> SerdeResult<Bound<'a, PyAny>> {
        // Never type should not have any values to dump
        invalid_type_dump!("Never", value)
    }

    #[inline]
    fn load<'a>(
        &self,
        value: &Bound<'a, PyAny>,
        instance_path: &InstancePath,
        _ctx: &Context,
    ) -> SerdeResult<Bound<'a, PyAny>> {
        // Never type cannot be loaded - any value is invalid
        invalid_type!("Never (no value allowed)", value, instance_path)
    }

    #[inline]
    fn dump_format(
        &self,
        value: &Bound<'_, PyAny>,
        _writer: &mut Writer,
        _ctx: &Context,
    ) -> SerdeResult<()> {
        // Never type should not have any values to dump
        invalid_type_dump!("Never", value)
    }

    #[inline]
    fn load_format<'py>(
        &self,
        _py: Python<'py>,
        parser: &mut Parser<'_>,
        instance_path: &InstancePath,
        _ctx: &Context,
    ) -> SerdeResult<Bound<'py, PyAny>> {
        // Never type cannot be loaded - error natively, no Python materialization.
        let raw = parser.take_raw_value()?;
        let raw = String::from_utf8_lossy(raw);
        Err(wrong_type_err(
            "Never (no value allowed)",
            &raw,
            instance_path,
        ))
    }
}

#[derive(Debug, Clone)]
pub struct IntEncoder {
    pub(crate) type_info: IntegerTypeInfo,
}

impl Encoder for IntEncoder {
    #[inline]
    fn dump<'a>(&self, value: &Bound<'a, PyAny>, _ctx: &Context) -> SerdeResult<Bound<'a, PyAny>> {
        Ok(value.clone())
    }

    #[inline]
    fn load<'a>(
        &self,
        value: &Bound<'a, PyAny>,
        instance_path: &InstancePath,
        ctx: &Context,
    ) -> SerdeResult<Bound<'a, PyAny>> {
        if let Ok(val) = value.cast_exact::<PyInt>() {
            check_bounds!(val.extract()?, self.type_info, instance_path)?;
            return Ok(value.clone());
        }
        if ctx.try_cast_from_string {
            if let Ok(val) = value.cast::<PyString>() {
                if let Ok(val) = val.to_str()?.parse::<i64>() {
                    check_bounds!(val, self.type_info, instance_path)?;
                    return Ok(val.into_bound_py_any(value.py())?);
                }
            }
        }
        invalid_type!("integer", value, instance_path)
    }

    fn dump_format(
        &self,
        value: &Bound<'_, PyAny>,
        writer: &mut Writer,
        _ctx: &Context,
    ) -> SerdeResult<()> {
        if let Ok(v) = value.cast_exact::<PyInt>() {
            match v.extract::<i64>() {
                Ok(i) => writer.write_i64(i),
                Err(_) => writer.write_big_int(v.str()?.to_str()?),
            }
            return Ok(());
        }
        invalid_type_dump!("integer", value)
    }

    // Decodes the integer straight from jiter (`take_int_known`) instead of a
    // text round-trip. jiter's `known_int` rejects a float-shaped token (e.g.
    // `1.5`) WITHOUT advancing the cursor, so on that error we re-read the raw
    // number text and defer to `load(float)` — producing the SchemaValidationError
    // ("not of type integer") the dict-path gives, not a DecodeError.
    fn load_format<'py>(
        &self,
        py: Python<'py>,
        parser: &mut Parser<'_>,
        instance_path: &InstancePath,
        ctx: &Context,
    ) -> SerdeResult<Bound<'py, PyAny>> {
        if parser.peek()? == Kind::Num {
            match parser.take_int_known() {
                Ok(ParsedInt::I64(v)) => {
                    check_bounds!(v, self.type_info, instance_path)?;
                    return Ok(v.into_bound_py_any(py)?);
                }
                Ok(ParsedInt::Big(big)) => {
                    // Unbounded: accept arbitrary-precision integers as-is
                    // (the i64 bounds-check in `load` would overflow on these).
                    if self.type_info.min.is_none() && self.type_info.max.is_none() {
                        return Ok(big.into_bound_py_any(py)?);
                    }
                    // Bounded: materialize and let `load` apply the standard
                    // (overflowing) bounds check, identical to the bridge default.
                    let materialized = big.into_bound_py_any(py)?;
                    return self.load(&materialized, instance_path, ctx);
                }
                Err(_) => {
                    // Float-shaped (or malformed) token: the cursor is unmoved, so
                    // re-read the raw number. A valid float defers to `load(float)`
                    // for the same "integer" schema error; a genuinely malformed
                    // number re-errors here as a DecodeError (same as before).
                    let raw = parser.take_number_str_known()?;
                    let v: f64 = raw.parse().map_err(|_| {
                        SerdeError::Py(ValidationError::new_err(format!("invalid number: {raw}")))
                    })?;
                    let materialized = PyFloat::new(py, v).into_any();
                    return self.load(&materialized, instance_path, ctx);
                }
            }
        }
        let raw = parser.take_raw_value()?;
        let raw = String::from_utf8_lossy(raw);
        Err(wrong_type_err("integer", &raw, instance_path))
    }
}

#[derive(Debug, Clone)]
pub struct FloatEncoder {
    pub(crate) type_info: FloatTypeInfo,
}

impl Encoder for FloatEncoder {
    #[inline]
    fn dump<'a>(&self, value: &Bound<'a, PyAny>, _ctx: &Context) -> SerdeResult<Bound<'a, PyAny>> {
        Ok(value.clone())
    }
    #[inline]
    fn load<'a>(
        &self,
        value: &Bound<'a, PyAny>,
        instance_path: &InstancePath,
        ctx: &Context,
    ) -> SerdeResult<Bound<'a, PyAny>> {
        if let Ok(val) = value.cast::<PyInt>() {
            check_bounds!(val.extract()?, self.type_info, instance_path)?;
            return Ok(value.clone());
        }
        if let Ok(val) = value.cast::<PyFloat>() {
            check_bounds!(val.extract()?, self.type_info, instance_path)?;
            return Ok(value.clone());
        }
        if ctx.try_cast_from_string {
            if let Ok(val) = value.cast::<PyString>() {
                if let Ok(val) = val.to_str()?.parse::<f64>() {
                    check_bounds!(val, self.type_info, instance_path)?;
                    return Ok(val.into_bound_py_any(value.py())?);
                }
            }
        }
        invalid_type!("number", value, instance_path)
    }

    fn dump_format(
        &self,
        value: &Bound<'_, PyAny>,
        writer: &mut Writer,
        _ctx: &Context,
    ) -> SerdeResult<()> {
        if let Ok(v) = value.cast::<PyFloat>() {
            writer
                .write_f64(v.value())
                .map_err(|msg| SerdeError::Py(ValidationError::new_err(msg)))?;
            return Ok(());
        }
        if let Ok(v) = value.cast::<PyInt>() {
            match v.extract::<i64>() {
                Ok(i) => writer.write_i64(i),
                Err(_) => writer.write_big_int(v.str()?.to_str()?),
            }
            return Ok(());
        }
        invalid_type_dump!("number", value)
    }

    // Integer-shaped tokens (no dot/exponent) are materialized as a Python int
    // and deferred to `self.load`, so a float field returns an int for `b'1'`,
    // exactly like the dict-path (`load(1)` returns `1`, not `1.0`). Float-shaped
    // tokens keep the fast direct-parse path below.
    fn load_format<'py>(
        &self,
        py: Python<'py>,
        parser: &mut Parser<'_>,
        instance_path: &InstancePath,
        ctx: &Context,
    ) -> SerdeResult<Bound<'py, PyAny>> {
        if parser.peek()? == Kind::Num {
            let raw = parser.take_number_str_known()?;
            if raw.bytes().all(|b| b.is_ascii_digit() || b == b'-') {
                let materialized = match raw.parse::<i64>() {
                    Ok(v) => v.into_bound_py_any(py)?,
                    Err(_) => {
                        let big: BigInt = raw.parse().map_err(|_| {
                            SerdeError::Py(ValidationError::new_err(format!(
                                "invalid number: {raw}"
                            )))
                        })?;
                        big.into_bound_py_any(py)?
                    }
                };
                return self.load(&materialized, instance_path, ctx);
            }
            if let Ok(v) = raw.parse::<f64>() {
                check_bounds!(v, self.type_info, instance_path)?;
                return Ok(PyFloat::new(py, v).into_any());
            }
            // Unreachable in practice: jiter only hands us syntactically valid
            // JSON number text, which always parses as f64. Kept as a safe,
            // non-panicking fallback with the same error `parse_any` would give.
            return Err(SerdeError::Py(ValidationError::new_err(format!(
                "invalid number: {raw}"
            ))));
        }
        let raw = parser.take_raw_value()?;
        let raw = String::from_utf8_lossy(raw);
        Err(wrong_type_err("number", &raw, instance_path))
    }
}

#[derive(Debug, Clone)]
pub struct DecimalEncoder {
    pub(crate) type_info: DecimalTypeInfo,
    pub(crate) decimal_cls: Py<PyAny>,
}

impl Encoder for DecimalEncoder {
    #[inline]
    fn dump<'a>(&self, value: &Bound<'a, PyAny>, _ctx: &Context) -> SerdeResult<Bound<'a, PyAny>> {
        Ok(value.str()?.into_any())
    }

    #[inline]
    fn load<'a>(
        &self,
        value: &Bound<'a, PyAny>,
        instance_path: &InstancePath,
        _ctx: &Context,
    ) -> SerdeResult<Bound<'a, PyAny>> {
        let valid = if let Ok(val) = value.cast::<PyFloat>() {
            check_bounds!(val.value(), self.type_info, instance_path)?;
            true
        } else if let Ok(val) = value.cast::<PyInt>() {
            check_bounds!(val.extract()?, self.type_info, instance_path)?;
            true
        } else if let Ok(val) = value.cast::<PyString>() {
            match val.to_str()?.parse::<f64>() {
                Ok(val_f64) => {
                    check_bounds!(val_f64, self.type_info, instance_path)?;
                    true
                }
                Err(_) => false,
            }
        } else {
            false
        };
        if valid {
            let str_value = value.str()?;
            Ok(self.decimal_cls.bind(value.py()).call1((str_value,))?)
        } else {
            invalid_type!("decimal", value, instance_path)
        }
    }

    #[inline]
    fn dump_format(
        &self,
        value: &Bound<'_, PyAny>,
        writer: &mut Writer,
        _ctx: &Context,
    ) -> SerdeResult<()> {
        writer.write_str(value.str()?.to_str()?);
        Ok(())
    }

    // Builds the Decimal straight from the raw JSON text (not through an f64
    // round-trip), so precision beyond what f64 can represent survives — e.g.
    // `load(b'1.1')` gives `Decimal('1.1')` instead of a float-repr artifact.
    // The f64 parse below is only used for the bounds check.
    fn load_format<'py>(
        &self,
        py: Python<'py>,
        parser: &mut Parser<'_>,
        instance_path: &InstancePath,
        ctx: &Context,
    ) -> SerdeResult<Bound<'py, PyAny>> {
        match parser.peek()? {
            Kind::Num => {
                let raw = parser.take_number_str_known()?;
                match raw.parse::<f64>() {
                    Ok(v) => {
                        check_bounds!(v, self.type_info, instance_path)?;
                        Ok(self.decimal_cls.bind(py).call1((PyString::new(py, raw),))?)
                    }
                    // Unreachable in practice: jiter only hands us syntactically
                    // valid JSON number text, which always parses as f64.
                    Err(_) => Err(SerdeError::Py(ValidationError::new_err(format!(
                        "invalid number: {raw}"
                    )))),
                }
            }
            Kind::Str => {
                let s = parser.take_str_known()?;
                match s.parse::<f64>() {
                    Ok(v) => {
                        check_bounds!(v, self.type_info, instance_path)?;
                        Ok(self.decimal_cls.bind(py).call1((PyString::new(py, s),))?)
                    }
                    Err(_) => {
                        // Not a decimal-shaped string -> same error as `load`.
                        let materialized = PyString::new(py, s).into_any();
                        self.load(&materialized, instance_path, ctx)
                    }
                }
            }
            _ => {
                let raw = parser.take_raw_value()?;
                let raw = String::from_utf8_lossy(raw);
                Err(wrong_type_err("decimal", &raw, instance_path))
            }
        }
    }
}

#[derive(Debug, Clone)]
pub struct StringEncoder {
    pub(crate) type_info: StringTypeInfo,
}

impl Encoder for StringEncoder {
    #[inline]
    fn dump<'a>(&self, value: &Bound<'a, PyAny>, _ctx: &Context) -> SerdeResult<Bound<'a, PyAny>> {
        Ok(value.clone())
    }

    #[inline]
    fn load<'a>(
        &self,
        value: &Bound<'a, PyAny>,
        instance_path: &InstancePath,
        _ctx: &Context,
    ) -> SerdeResult<Bound<'a, PyAny>> {
        if let Ok(val) = value.cast::<PyString>() {
            check_length(
                val,
                self.type_info.min_length,
                self.type_info.max_length,
                instance_path,
            )?;
            Ok(value.clone())
        } else {
            invalid_type!("string", value, instance_path)
        }
    }

    #[inline]
    fn dump_format(
        &self,
        value: &Bound<'_, PyAny>,
        writer: &mut Writer,
        _ctx: &Context,
    ) -> SerdeResult<()> {
        if let Ok(v) = value.cast::<PyString>() {
            writer.write_str(v.to_str()?);
            return Ok(());
        }
        invalid_type_dump!("string", value)
    }

    #[inline]
    fn load_format<'py>(
        &self,
        py: Python<'py>,
        parser: &mut Parser<'_>,
        instance_path: &InstancePath,
        _ctx: &Context,
    ) -> SerdeResult<Bound<'py, PyAny>> {
        if parser.peek()? == Kind::Str {
            let s = parser.take_str_known()?;
            let py_str = PyString::new(py, s);
            check_length(
                &py_str,
                self.type_info.min_length,
                self.type_info.max_length,
                instance_path,
            )?;
            return Ok(py_str.into_any());
        }
        let raw = parser.take_raw_value()?;
        let raw = String::from_utf8_lossy(raw);
        Err(wrong_type_err("string", &raw, instance_path))
    }
}

#[derive(Debug, Clone)]
pub struct BooleanEncoder {}

impl Encoder for BooleanEncoder {
    #[inline]
    fn dump<'a>(&self, value: &Bound<'a, PyAny>, _ctx: &Context) -> SerdeResult<Bound<'a, PyAny>> {
        Ok(value.clone())
    }

    #[inline]
    fn load<'a>(
        &self,
        value: &Bound<'a, PyAny>,
        instance_path: &InstancePath,
        ctx: &Context,
    ) -> SerdeResult<Bound<'a, PyAny>> {
        if let Ok(_val) = value.cast::<PyBool>() {
            return Ok(value.clone());
        }
        if ctx.try_cast_from_string {
            if let Ok(val) = value.cast::<PyString>() {
                if let Some(val) = str_as_bool(val.to_str()?) {
                    return Ok(val.into_bound_py_any(value.py())?);
                }
            }
        }

        invalid_type!("boolean", value, instance_path)
    }

    #[inline]
    fn dump_format(
        &self,
        value: &Bound<'_, PyAny>,
        writer: &mut Writer,
        _ctx: &Context,
    ) -> SerdeResult<()> {
        if let Ok(v) = value.cast::<PyBool>() {
            writer.write_bool(v.is_true());
            return Ok(());
        }
        invalid_type_dump!("boolean", value)
    }

    #[inline]
    fn load_format<'py>(
        &self,
        py: Python<'py>,
        parser: &mut Parser<'_>,
        instance_path: &InstancePath,
        _ctx: &Context,
    ) -> SerdeResult<Bound<'py, PyAny>> {
        if parser.peek()? == Kind::Bool {
            let b = parser.take_bool_known()?;
            return Ok(PyBool::new(py, b).to_owned().into_any());
        }
        let raw = parser.take_raw_value()?;
        let raw = String::from_utf8_lossy(raw);
        Err(wrong_type_err("boolean", &raw, instance_path))
    }
}

#[derive(Debug, Clone)]
pub struct BytesEncoder {}

impl Encoder for BytesEncoder {
    #[inline]
    fn dump<'a>(&self, value: &Bound<'a, PyAny>, _ctx: &Context) -> SerdeResult<Bound<'a, PyAny>> {
        Ok(value.clone())
    }

    #[inline]
    fn load<'a>(
        &self,
        value: &Bound<'a, PyAny>,
        instance_path: &InstancePath,
        _ctx: &Context,
    ) -> SerdeResult<Bound<'a, PyAny>> {
        if let Ok(_val) = value.cast::<PyBytes>() {
            Ok(value.clone())
        } else {
            invalid_type!("bytes", value, instance_path)
        }
    }

    // JSON has no bytes; give a clear error instead of the generic bridge
    // "not serializable" message. load_format uses the default: parse_any
    // yields a non-bytes value and `load` returns the same "bytes" error.
    fn dump_format(
        &self,
        _value: &Bound<'_, PyAny>,
        _writer: &mut Writer,
        _ctx: &Context,
    ) -> SerdeResult<()> {
        Err(SerdeError::Py(ValidationError::new_err(
            "bytes values are not supported by this format".to_string(),
        )))
    }
}

/// Write a dumped dict key as a map key, mirroring `bridge::write_any`'s key
/// handling: string keys go straight through, everything else via `str()`.
#[inline]
fn write_map_key(key: &Bound<'_, PyAny>, writer: &mut Writer) -> SerdeResult<()> {
    match key.cast::<PyString>() {
        Ok(s) => writer.map_key(s.to_str()?),
        Err(_) => writer.map_key(key.str()?.to_str()?),
    }
    Ok(())
}

/// Write an already-resolved enum/literal serialized value by its concrete
/// Python type. The common str/int members stream directly, keeping them off
/// the generic `write_any` bridge; anything unusual (float, None, container
/// literal member, ...) falls back to `write_any`, so the output is
/// byte-identical to the bridge for every possible value.
fn write_scalar_item(
    item: &Bound<'_, PyAny>,
    writer: &mut Writer,
    ctx: &Context,
) -> SerdeResult<()> {
    if let Ok(v) = item.cast::<PyString>() {
        writer.write_str(v.to_str()?);
        return Ok(());
    }
    // bool is a subtype of int in Python, so it must be checked first.
    if let Ok(v) = item.cast::<PyBool>() {
        writer.write_bool(v.is_true());
        return Ok(());
    }
    if let Ok(v) = item.cast::<PyInt>() {
        match v.extract::<i64>() {
            Ok(i) => writer.write_i64(i),
            Err(_) => writer.write_big_int(v.str()?.to_str()?),
        }
        return Ok(());
    }
    if let Ok(v) = item.cast::<PyFloat>() {
        writer
            .write_f64(v.value())
            .map_err(|msg| SerdeError::Py(ValidationError::new_err(msg)))?;
        return Ok(());
    }
    // Exotic value (None / container / other): full-fidelity fallback.
    write_any(item, writer, ctx)
}

#[derive(Debug, Clone)]
pub struct DictionaryEncoder {
    pub(crate) key_encoder: Box<TEncoder>,
    pub(crate) value_encoder: Box<TEncoder>,
    pub(crate) omit_none: bool,
    /// True when the key type is a plain `str` (no min/max length, no custom
    /// encoder): `key_encoder.load` would just re-validate and clone the same
    /// string, so the streaming load path uses the parsed key directly.
    pub(crate) key_is_plain_str: bool,
}

impl Encoder for DictionaryEncoder {
    #[inline]
    fn dump<'a>(&self, value: &Bound<'a, PyAny>, ctx: &Context) -> SerdeResult<Bound<'a, PyAny>> {
        let _guard = ctx.enter_depth()?;
        if let Ok(dict) = value.cast::<PyDict>() {
            let result_dict = create_py_dict_known_size(dict.py(), dict.len())?;
            for (k, v) in dict.iter() {
                let key = self.key_encoder.dump(&k, ctx)?;
                let value = self.value_encoder.dump(&v, ctx)?;
                if !self.omit_none || !value.is_none() {
                    py_dict_set_item(&result_dict, key.as_ptr(), value)?;
                }
            }
            Ok(result_dict.into_any())
        } else {
            invalid_type_dump!("dict", value)
        }
    }

    #[inline]
    fn load<'a>(
        &self,
        value: &Bound<'a, PyAny>,
        instance_path: &InstancePath,
        ctx: &Context,
    ) -> SerdeResult<Bound<'a, PyAny>> {
        let _guard = ctx.enter_depth()?;
        if let Ok(val) = value.cast::<PyDict>() {
            let result_dict = create_py_dict_known_size(val.py(), val.len())?;
            for (k, v) in val.iter() {
                let instance_path = instance_path.push(&k);
                let key = self.key_encoder.load(&k, &instance_path, ctx)?;
                let value = self.value_encoder.load(&v, &instance_path, ctx)?;
                py_dict_set_item(&result_dict, key.as_ptr(), value)?;
            }
            Ok(result_dict.into_any())
        } else {
            invalid_type_dump!("dict", value)
        }
    }

    #[inline]
    fn dump_format(
        &self,
        value: &Bound<'_, PyAny>,
        writer: &mut Writer,
        ctx: &Context,
    ) -> SerdeResult<()> {
        let _guard = ctx.enter_depth()?;
        if let Ok(dict) = value.cast::<PyDict>() {
            writer.begin_map();
            for (k, v) in dict.iter() {
                if self.omit_none {
                    // omit_none needs to know whether the dumped value is None
                    // before the key is written. Stream the value into a probe
                    // via the concrete encoder: it writes exactly `null` iff the
                    // dumped value is None, so the byte check is equivalent to
                    // the dict-path `is_none()` without materializing via dump.
                    let mut probe = writer.new_probe();
                    self.value_encoder.dump_format(&v, &mut probe, ctx)?;
                    if probe.as_bytes() == b"null" {
                        continue;
                    }
                    let key = self.key_encoder.dump(&k, ctx)?;
                    write_map_key(&key, writer)?;
                    writer.write_raw_value(probe.as_bytes());
                } else {
                    let key = self.key_encoder.dump(&k, ctx)?;
                    write_map_key(&key, writer)?;
                    self.value_encoder.dump_format(&v, writer, ctx)?;
                }
            }
            writer.end_map();
            Ok(())
        } else {
            invalid_type_dump!("dict", value)
        }
    }

    fn load_format<'py>(
        &self,
        py: Python<'py>,
        parser: &mut Parser<'_>,
        instance_path: &InstancePath,
        ctx: &Context,
    ) -> SerdeResult<Bound<'py, PyAny>> {
        let _guard = ctx.enter_depth()?;
        if parser.peek()? != Kind::Map {
            let raw = parser.take_raw_value()?;
            let raw = String::from_utf8_lossy(raw);
            return Err(wrong_type_err("dict", &raw, instance_path));
        }
        let result_dict = PyDict::new(py);
        // The key `&str` borrows the parser buffer. Materialize it into a
        // `Bound<PyString>` immediately (PyString::new copies the bytes), which
        // ends the borrow so the parser is free for the value's `load_format`.
        // The owned PyString then serves both the instance_path (as a
        // `PropertyValue` chunk — no `String` alloc) and the dict insert.
        let mut key_opt = parser.enter_map_known()?;
        while let Some(k) = key_opt {
            let py_key = PyString::new(py, k);
            let key_any = py_key.as_any();
            let item_path = instance_path.push(key_any);
            // Plain-str keys skip `key_encoder.load`: it would only re-check the
            // (absent) length bounds and clone the same string we already hold.
            // `validated_key` keeps the key object alive past the dict insert in
            // the non-plain case (the fast case uses `py_key`, alive anyway).
            let validated_key;
            let key_ptr = if self.key_is_plain_str {
                key_any.as_ptr()
            } else {
                validated_key = self.key_encoder.load(key_any, &item_path, ctx)?;
                validated_key.as_ptr()
            };
            let py_value = self
                .value_encoder
                .load_format(py, parser, &item_path, ctx)?;
            py_dict_set_item(&result_dict, key_ptr, py_value)?;
            key_opt = parser.next_key()?;
        }
        Ok(result_dict.into_any())
    }

    fn as_container_encoder(&self) -> Option<&dyn ContainerEncoder> {
        Some(self)
    }
}

impl ContainerEncoder for DictionaryEncoder {
    fn get_fields(&self) -> QueryFields<'_> {
        QueryFields::Dict(self.value_encoder.is_sequence())
    }
}

#[derive(Debug, Clone)]
pub struct ArrayEncoder {
    pub(crate) encoder: Box<TEncoder>,
    pub(crate) min_length: Option<usize>,
    pub(crate) max_length: Option<usize>,
}

impl Encoder for ArrayEncoder {
    #[inline]
    fn dump<'a>(&self, value: &Bound<'a, PyAny>, ctx: &Context) -> SerdeResult<Bound<'a, PyAny>> {
        let _guard = ctx.enter_depth()?;
        if let Ok(list) = value.cast::<PyList>() {
            let size = list.len();
            let result = create_py_list(value.py(), size)?;

            for index in 0..size {
                let item = py_list_get_item(list, index)?;
                let val = self.encoder.dump(&item, ctx)?;
                py_list_set_item(&result, index, val);
            }

            Ok(result.into_any())
        } else {
            invalid_type_dump!("list", value)
        }
    }

    #[inline]
    fn load<'a>(
        &self,
        value: &Bound<'a, PyAny>,
        instance_path: &InstancePath,
        ctx: &Context,
    ) -> SerdeResult<Bound<'a, PyAny>> {
        let _guard = ctx.enter_depth()?;
        if let Ok(val) = value.cast::<PyList>() {
            let size = val.len();
            check_sequence_bounds(
                val,
                size,
                self.min_length,
                self.max_length,
                Some(instance_path),
            )?;
            let result = create_py_list(value.py(), size)?;

            for index in 0..size {
                let item = py_list_get_item(val, index)?;
                let instance_path = instance_path.push(index);
                let val = self.encoder.load(&item, &instance_path, ctx)?;
                py_list_set_item(&result, index, val);
            }
            Ok(result.into_any())
        } else {
            invalid_type!("list", value, instance_path)
        }
    }

    #[inline]
    fn dump_format(
        &self,
        value: &Bound<'_, PyAny>,
        writer: &mut Writer,
        ctx: &Context,
    ) -> SerdeResult<()> {
        let _guard = ctx.enter_depth()?;
        if let Ok(list) = value.cast::<PyList>() {
            writer.begin_array();
            for index in 0..list.len() {
                writer.array_item();
                let item = py_list_get_item(list, index)?;
                self.encoder.dump_format(&item, writer, ctx)?;
            }
            writer.end_array();
            Ok(())
        } else {
            invalid_type_dump!("list", value)
        }
    }

    fn load_format<'py>(
        &self,
        py: Python<'py>,
        parser: &mut Parser<'_>,
        instance_path: &InstancePath,
        ctx: &Context,
    ) -> SerdeResult<Bound<'py, PyAny>> {
        let _guard = ctx.enter_depth()?;
        if parser.peek()? != Kind::Array {
            let raw = parser.take_raw_value()?;
            let raw = String::from_utf8_lossy(raw);
            return Err(wrong_type_err("list", &raw, instance_path));
        }
        let mut items: Vec<Bound<'py, PyAny>> = Vec::new();
        if parser.enter_array_known()? {
            loop {
                // Length bounds can only be checked after the closing bracket
                // is seen, so an element-type error here surfaces before a
                // would-be length error (unlike the dict path, which checks
                // length against the raw input up front).
                let item_path = instance_path.push(items.len());
                items.push(self.encoder.load_format(py, parser, &item_path, ctx)?);
                if !parser.next_array_item()? {
                    break;
                }
            }
        }
        let list = PyList::new(py, items)?;
        check_sequence_bounds(
            &list,
            list.len(),
            self.min_length,
            self.max_length,
            Some(instance_path),
        )?;
        Ok(list.into_any())
    }

    fn is_sequence(&self) -> bool {
        true
    }
}

/// Routing decision for one streamed object key, computed while the borrowed
/// `&str` key is still alive so the key never has to be copied to an owned
/// `String`. `Copy`, so it outlives the parser borrow that produced it.
#[derive(Clone, Copy)]
enum Route {
    /// Key maps to `self.fields[idx]`.
    Field(usize),
    /// Unknown key — skip its value.
    Skip,
    /// End of object.
    End,
}

/// Resolve a borrowed key to a `Route` (no allocation). `None` (end of object)
/// -> `End`; a known key -> `Field(idx)`; anything else -> `Skip`.
#[inline]
fn resolve_route(routing: &FxHashMap<String, usize>, key: Option<&str>) -> Route {
    match key {
        Some(k) => match routing.get(k) {
            Some(&idx) => Route::Field(idx),
            None => Route::Skip,
        },
        None => Route::End,
    }
}

#[derive(Debug, Clone)]
pub struct EntityEncoder {
    pub(crate) cls: Py<PyType>,
    pub(crate) omit_none: bool,
    pub(crate) is_frozen: bool,
    pub(crate) fields: Vec<Field>,
    pub(crate) used_keys: Py<PySet>,
    /// Maps JSON key (dict_key_rs) -> field index. Non-flatten fields only.
    /// Empty is fine; used only by the streaming (no-flatten) load path.
    pub(crate) format_routing: FxHashMap<String, usize>,
    /// Cached `fields.iter().any(|f| f.is_flattened)`, computed once at
    /// construction so the format hot paths don't rescan on every call.
    pub(crate) has_flatten: bool,
}

#[derive(Debug, Clone)]
pub struct Field {
    pub(crate) name: Py<PyString>,
    pub(crate) dict_key: Py<PyString>,
    pub(crate) dict_key_rs: String,
    pub(crate) encoder: Box<TEncoder>,
    pub(crate) required: bool,
    pub(crate) default: Option<Py<PyAny>>,
    pub(crate) default_factory: Option<Py<PyAny>>,
    pub(crate) is_flattened: bool,
    pub(crate) is_dict_flatten: bool,
}

impl Field {
    pub(crate) fn get_default<'a>(
        &self,
        py: Python<'a>,
        instance_path: &InstancePath,
    ) -> SerdeResult<Bound<'a, PyAny>> {
        match (&self.default, &self.default_factory) {
            (Some(val), _) => Ok(val.bind(py).clone()),
            (_, Some(factory)) => Ok(factory.bind(py).call0()?),
            (None, _) => Err(missing_required_property(&self.dict_key_rs, instance_path)),
        }
    }

    pub(crate) fn load_value<'a>(
        &self,
        val: &Bound<'a, PyDict>,
        instance_path: &InstancePath,
        ctx: &Context,
        used_keys: &Py<PySet>,
    ) -> SerdeResult<Bound<'a, PyAny>> {
        if self.is_flattened {
            if self.is_dict_flatten {
                let remaining_dict = create_remaining_dict(val, used_keys)?;
                self.encoder.load(&remaining_dict, instance_path, ctx)
            } else {
                self.encoder.load(val, instance_path, ctx)
            }
        } else {
            match val.get_item(&self.dict_key)? {
                Some(field_val) => {
                    let field_instance_path =
                        instance_path.push(self.dict_key.bind(val.py()).as_any());
                    self.encoder.load(&field_val, &field_instance_path, ctx)
                }
                None => self.get_default(val.py(), instance_path),
            }
        }
    }
}

impl Encoder for EntityEncoder {
    #[inline]
    fn dump<'a>(&self, value: &Bound<'a, PyAny>, ctx: &Context) -> SerdeResult<Bound<'a, PyAny>> {
        let _guard = ctx.enter_depth()?;
        let dict = create_py_dict_known_size(value.py(), self.fields.len())?;
        for field in &self.fields {
            let field_val = value.getattr(&field.name)?;
            let dump_result = field.encoder.dump(&field_val, ctx)?;
            if field.required || !self.omit_none || !dump_result.is_none() {
                if field.is_flattened {
                    let mapping = dump_result
                        .cast::<pyo3::types::PyMapping>()
                        .map_err(PyErr::from)?;
                    dict.update(mapping)?;
                } else {
                    py_dict_set_item(&dict, field.dict_key.as_ptr(), dump_result)?;
                }
            }
        }
        Ok(dict.into_any())
    }

    #[inline]
    fn load<'a>(
        &self,
        value: &Bound<'a, PyAny>,
        instance_path: &InstancePath,
        ctx: &Context,
    ) -> SerdeResult<Bound<'a, PyAny>> {
        let _guard = ctx.enter_depth()?;
        let Ok(val) = value.cast::<PyDict>() else {
            invalid_type!("object", value, instance_path)
        };
        let obj = create_instance(self.cls.bind(value.py()))?;

        for field in &self.fields {
            let val = field.load_value(val, instance_path, ctx, &self.used_keys)?;
            if self.is_frozen {
                generic_set_attr(&obj, field.name.as_ptr(), val)?;
            } else {
                set_attr_unchecked(&obj, field.name.as_ptr(), val)?;
            };
        }

        Ok(obj)
    }

    // Streams the object directly to the writer, avoiding the intermediate
    // PyDict the dict-path (dump) builds. Flatten entities keep full parity via
    // the bridge (materialize + write_any) — streaming flatten is a future
    // optimization.
    fn dump_format(
        &self,
        value: &Bound<'_, PyAny>,
        writer: &mut Writer,
        ctx: &Context,
    ) -> SerdeResult<()> {
        if self.has_flatten {
            let dumped = self.dump(value, ctx)?;
            return write_any(&dumped, writer, ctx);
        }
        let _guard = ctx.enter_depth()?;
        writer.begin_map();
        for field in &self.fields {
            // A missing attribute means the value isn't an instance of this
            // entity's shape. Surface it as a Schema type-mismatch (like the
            // scalar/container dump_format guards) so an enclosing untagged
            // union skips to the next member, instead of a raw AttributeError
            // that would abort union probing.
            let field_val = match value.getattr(&field.name) {
                Ok(v) => v,
                Err(e) if e.is_instance_of::<PyAttributeError>(value.py()) => {
                    let name = self.cls.bind(value.py()).name()?;
                    return Err(invalid_type_dump_err(&name.to_string(), value));
                }
                Err(e) => return Err(e.into()),
            };
            // Mirror the dict-path write condition
            // (`field.required || !self.omit_none || !dump_result.is_none()`):
            // only optional fields under omit_none need the dumped value first
            // to decide whether to emit the key at all.
            if !field.required && self.omit_none {
                // Stream the value into a probe via the concrete encoder: it
                // writes exactly `null` iff the dumped value is None, so the
                // byte check is equivalent to the dict-path `is_none()` without
                // materializing via dump + write_any.
                let mut probe = writer.new_probe();
                field.encoder.dump_format(&field_val, &mut probe, ctx)?;
                if probe.as_bytes() != b"null" {
                    writer.map_key(&field.dict_key_rs);
                    writer.write_raw_value(probe.as_bytes());
                }
            } else {
                writer.map_key(&field.dict_key_rs);
                field.encoder.dump_format(&field_val, writer, ctx)?;
            }
        }
        writer.end_map();
        Ok(())
    }

    // Streams straight into a class instance, avoiding the intermediate PyDict
    // the dict-path (load) parses first. Keys are routed via `format_routing`;
    // unknown keys are skipped. Flatten entities stream natively too: routed
    // keys are loaded straight into the instance as usual, while keys not in
    // `format_routing` (i.e. destined for a flatten field) are materialized
    // only as their individual values into an `unknowns` dict — never the
    // whole object — which is then handed to each flatten field's existing
    // `Field::load_value` (shared with the dict path) to resolve.
    fn load_format<'py>(
        &self,
        py: Python<'py>,
        parser: &mut Parser<'_>,
        instance_path: &InstancePath,
        ctx: &Context,
    ) -> SerdeResult<Bound<'py, PyAny>> {
        if self.has_flatten {
            let _guard = ctx.enter_depth()?;
            if parser.peek()? != Kind::Map {
                let raw = parser.take_raw_value()?;
                let raw = String::from_utf8_lossy(raw);
                return Err(wrong_type_err("object", &raw, instance_path));
            }
            let obj = create_instance(self.cls.bind(py))?;
            let n = self.fields.len();
            let mut seen: SmallVec<[u64; 1]> = smallvec![0u64; n.div_ceil(64)];
            let unknowns = PyDict::new(py);
            let mut key = parser.enter_map_known()?;
            while let Some(k) = key {
                match self.format_routing.get(k) {
                    Some(&idx) => {
                        let field = &self.fields[idx];
                        let field_path = instance_path.push(field.dict_key_rs.as_str());
                        let val = field.encoder.load_format(py, parser, &field_path, ctx)?;
                        if self.is_frozen {
                            generic_set_attr(&obj, field.name.as_ptr(), val)?;
                        } else {
                            set_attr_unchecked(&obj, field.name.as_ptr(), val)?;
                        }
                        seen[idx >> 6] |= 1u64 << (idx & 63);
                    }
                    None => {
                        // Unknown key -> destined for a flatten field. Materialize
                        // only this value (not the whole object) into `unknowns`.
                        let py_key = PyString::new(py, k);
                        let v = parse_any(py, parser, ctx)?;
                        unknowns.set_item(py_key, v)?;
                    }
                }
                key = parser.next_key()?;
            }
            for (idx, field) in self.fields.iter().enumerate() {
                let val = if field.is_flattened {
                    field.load_value(&unknowns, instance_path, ctx, &self.used_keys)?
                } else if seen[idx >> 6] & (1u64 << (idx & 63)) == 0 {
                    field.get_default(py, instance_path)?
                } else {
                    continue; // already set from the stream
                };
                if self.is_frozen {
                    generic_set_attr(&obj, field.name.as_ptr(), val)?;
                } else {
                    set_attr_unchecked(&obj, field.name.as_ptr(), val)?;
                }
            }
            return Ok(obj);
        }
        let _guard = ctx.enter_depth()?;
        if parser.peek()? != Kind::Map {
            let raw = parser.take_raw_value()?;
            let raw = String::from_utf8_lossy(raw);
            return Err(wrong_type_err("object", &raw, instance_path));
        }
        let obj = create_instance(self.cls.bind(py))?;
        let n = self.fields.len();
        // Set attributes directly as keys arrive (no per-entity
        // `Vec<Option<Bound>>` — a hot alloc/free on entity-heavy loads),
        // tracking which fields were seen in an inline bitset that lives on the
        // stack for the common case (<= 64 fields => one word) and spills to the
        // heap only for very wide entities. Defaults then fill unseen fields.
        // Keys resolve to a Copy `Route` while borrowed, so they are never
        // copied to owned Strings.
        let mut seen: SmallVec<[u64; 1]> = smallvec![0u64; n.div_ceil(64)];
        let mut route = resolve_route(&self.format_routing, parser.enter_map_known()?);
        loop {
            match route {
                Route::End => break,
                Route::Field(idx) => {
                    let field = &self.fields[idx];
                    let field_path = instance_path.push(field.dict_key_rs.as_str());
                    let val = field.encoder.load_format(py, parser, &field_path, ctx)?;
                    if self.is_frozen {
                        generic_set_attr(&obj, field.name.as_ptr(), val)?;
                    } else {
                        set_attr_unchecked(&obj, field.name.as_ptr(), val)?;
                    }
                    seen[idx >> 6] |= 1u64 << (idx & 63);
                }
                Route::Skip => parser.skip_value()?,
            }
            route = resolve_route(&self.format_routing, parser.next_key()?);
        }
        for (idx, field) in self.fields.iter().enumerate() {
            if seen[idx >> 6] & (1u64 << (idx & 63)) == 0 {
                // Missing field: same default / missing-required error as the
                // dict-path (get_default pushes dict_key_rs onto the base path).
                let val = field.get_default(py, instance_path)?;
                if self.is_frozen {
                    generic_set_attr(&obj, field.name.as_ptr(), val)?;
                } else {
                    set_attr_unchecked(&obj, field.name.as_ptr(), val)?;
                }
            }
        }
        Ok(obj)
    }

    fn as_container_encoder(&self) -> Option<&dyn ContainerEncoder> {
        Some(self)
    }
}

fn create_remaining_dict<'a>(
    val: &Bound<'a, PyDict>,
    used_keys: &Py<PySet>,
) -> PyResult<Bound<'a, PyDict>> {
    let used_keys_set = used_keys.bind(val.py());
    let len = val.len().saturating_sub(used_keys_set.len());
    let remaining_dict = create_py_dict_known_size(val.py(), len)?;
    for (k, v) in val.iter() {
        if !used_keys_set.contains(&k)? {
            remaining_dict.set_item(k, v)?;
        }
    }
    Ok(remaining_dict)
}

fn get_fields_query(fields: &[Field]) -> QueryFields<'_> {
    QueryFields::Object(
        fields
            .iter()
            .map(|f| EncoderField {
                name: &f.dict_key,
                is_sequence: f.encoder.is_sequence(),
            })
            .collect(),
    )
}

impl ContainerEncoder for EntityEncoder {
    fn get_fields(&self) -> QueryFields<'_> {
        get_fields_query(&self.fields)
    }
}

#[derive(Debug, Clone)]
pub struct TypedDictEncoder {
    pub(crate) omit_none: bool,
    pub(crate) fields: Vec<Field>,
    pub(crate) used_keys: Py<PySet>,
    /// Maps JSON key (dict_key_rs) -> field index. Non-flatten fields only.
    /// Empty is fine; used only by the streaming (no-flatten) load path.
    pub(crate) format_routing: FxHashMap<String, usize>,
    /// Cached `fields.iter().any(|f| f.is_flattened)`, computed once at
    /// construction so the format hot paths don't rescan on every call.
    pub(crate) has_flatten: bool,
}

impl Encoder for TypedDictEncoder {
    #[inline]
    fn dump<'a>(&self, value: &Bound<'a, PyAny>, ctx: &Context) -> SerdeResult<Bound<'a, PyAny>> {
        let _guard = ctx.enter_depth()?;
        let value = match value.cast::<PyDict>() {
            Ok(val) => val,
            _ => invalid_type_dump!("dict", value),
        };
        let dict = create_py_dict_known_size(value.py(), self.fields.len())?;
        for field in &self.fields {
            let field_val = match value.get_item(&field.name) {
                Ok(Some(val)) => val,
                _ => {
                    if field.required {
                        return Err(SerdeError::Py(ValidationError::new_err(format!(
                            "data dictionary is missing required parameter {}",
                            field.name
                        ))));
                    }
                    continue;
                }
            };
            let dump_result = field.encoder.dump(&field_val, ctx)?;
            if field.required || !self.omit_none || !dump_result.is_none() {
                if field.is_flattened {
                    let mapping = dump_result
                        .cast::<pyo3::types::PyMapping>()
                        .map_err(PyErr::from)?;
                    dict.update(mapping)?;
                } else {
                    py_dict_set_item(&dict, field.dict_key.as_ptr(), dump_result)?;
                }
            }
        }
        Ok(dict.into_any())
    }

    #[inline]
    fn load<'a>(
        &self,
        value: &Bound<'a, PyAny>,
        instance_path: &InstancePath,
        ctx: &Context,
    ) -> SerdeResult<Bound<'a, PyAny>> {
        let _guard = ctx.enter_depth()?;
        let Ok(value) = value.cast::<PyDict>() else {
            invalid_type_dump!("dict", value);
        };
        let dict = create_py_dict_known_size(value.py(), self.fields.len())?;
        for field in &self.fields {
            let val = field.load_value(value, instance_path, ctx, &self.used_keys)?;
            py_dict_set_item(&dict, field.name.as_ptr(), val)?;
        }
        Ok(dict.into_any())
    }

    // Streams the mapping directly to the writer, avoiding the intermediate
    // PyDict the dict-path (dump) builds. Missing/optional/required handling
    // mirrors the dict-path exactly: a missing required key errors, a missing
    // optional key is skipped. Flatten typeddicts fall back to the bridge.
    fn dump_format(
        &self,
        value: &Bound<'_, PyAny>,
        writer: &mut Writer,
        ctx: &Context,
    ) -> SerdeResult<()> {
        if self.has_flatten {
            let dumped = self.dump(value, ctx)?;
            return write_any(&dumped, writer, ctx);
        }
        let _guard = ctx.enter_depth()?;
        let value = match value.cast::<PyDict>() {
            Ok(val) => val,
            _ => invalid_type_dump!("dict", value),
        };
        writer.begin_map();
        for field in &self.fields {
            let field_val = match value.get_item(&field.name) {
                Ok(Some(val)) => val,
                _ => {
                    if field.required {
                        return Err(SerdeError::Py(ValidationError::new_err(format!(
                            "data dictionary is missing required parameter {}",
                            field.name
                        ))));
                    }
                    // Missing optional key: skip entirely (no key emitted).
                    continue;
                }
            };
            // Mirror the dict-path write condition
            // (`field.required || !self.omit_none || !dump_result.is_none()`).
            if !field.required && self.omit_none {
                // Stream the value into a probe via the concrete encoder: it
                // writes exactly `null` iff the dumped value is None, so the
                // byte check is equivalent to the dict-path `is_none()` without
                // materializing via dump + write_any.
                let mut probe = writer.new_probe();
                field.encoder.dump_format(&field_val, &mut probe, ctx)?;
                if probe.as_bytes() != b"null" {
                    writer.map_key(&field.dict_key_rs);
                    writer.write_raw_value(probe.as_bytes());
                }
            } else {
                writer.map_key(&field.dict_key_rs);
                field.encoder.dump_format(&field_val, writer, ctx)?;
            }
        }
        writer.end_map();
        Ok(())
    }

    // Streams straight into the result PyDict, avoiding the intermediate PyDict
    // the dict-path (load) parses first. Keys are routed via `format_routing`;
    // unknown keys are skipped. Every field is written to the result (present
    // value or its default), matching the dict-path. Flatten typeddicts stream
    // natively too: routed keys are loaded straight into the result dict as
    // usual, while keys not in `format_routing` (i.e. destined for a flatten
    // field) are materialized only as their individual values into an
    // `unknowns` dict — never the whole object — which is then handed to each
    // flatten field's existing `Field::load_value` (shared with the dict path)
    // to resolve.
    fn load_format<'py>(
        &self,
        py: Python<'py>,
        parser: &mut Parser<'_>,
        instance_path: &InstancePath,
        ctx: &Context,
    ) -> SerdeResult<Bound<'py, PyAny>> {
        if self.has_flatten {
            let _guard = ctx.enter_depth()?;
            if parser.peek()? != Kind::Map {
                let raw = parser.take_raw_value()?;
                let raw = String::from_utf8_lossy(raw);
                return Err(wrong_type_err("dict", &raw, instance_path));
            }
            let dict = create_py_dict_known_size(py, self.fields.len())?;
            let n = self.fields.len();
            let mut seen: SmallVec<[u64; 1]> = smallvec![0u64; n.div_ceil(64)];
            let unknowns = PyDict::new(py);
            let mut key = parser.enter_map_known()?;
            while let Some(k) = key {
                match self.format_routing.get(k) {
                    Some(&idx) => {
                        let field = &self.fields[idx];
                        let field_path = instance_path.push(field.dict_key_rs.as_str());
                        let val = field.encoder.load_format(py, parser, &field_path, ctx)?;
                        py_dict_set_item(&dict, field.name.as_ptr(), val)?;
                        seen[idx >> 6] |= 1u64 << (idx & 63);
                    }
                    None => {
                        // Unknown key -> destined for a flatten field. Materialize
                        // only this value (not the whole object) into `unknowns`.
                        let py_key = PyString::new(py, k);
                        let v = parse_any(py, parser, ctx)?;
                        unknowns.set_item(py_key, v)?;
                    }
                }
                key = parser.next_key()?;
            }
            for (idx, field) in self.fields.iter().enumerate() {
                let val = if field.is_flattened {
                    field.load_value(&unknowns, instance_path, ctx, &self.used_keys)?
                } else if seen[idx >> 6] & (1u64 << (idx & 63)) == 0 {
                    field.get_default(py, instance_path)?
                } else {
                    continue; // already inserted from the stream
                };
                py_dict_set_item(&dict, field.name.as_ptr(), val)?;
            }
            return Ok(dict.into_any());
        }
        let _guard = ctx.enter_depth()?;
        if parser.peek()? != Kind::Map {
            let raw = parser.take_raw_value()?;
            let raw = String::from_utf8_lossy(raw);
            return Err(wrong_type_err("dict", &raw, instance_path));
        }
        let dict = create_py_dict_known_size(py, self.fields.len())?;
        let n = self.fields.len();
        // Insert values directly into the result dict as keys arrive (no
        // per-load `Vec<Option<Bound>>` — a hot alloc/free on typeddict-heavy
        // loads), tracking which fields were seen in an inline bitset that
        // lives on the stack for the common case (<= 64 fields => one word)
        // and spills to the heap only for very wide typeddicts. Defaults then
        // fill unseen fields. Keys resolve to a Copy `Route` while borrowed,
        // so they are never copied to owned Strings.
        let mut seen: SmallVec<[u64; 1]> = smallvec![0u64; n.div_ceil(64)];
        let mut route = resolve_route(&self.format_routing, parser.enter_map_known()?);
        loop {
            match route {
                Route::End => break,
                Route::Field(idx) => {
                    let field = &self.fields[idx];
                    let field_path = instance_path.push(field.dict_key_rs.as_str());
                    let val = field.encoder.load_format(py, parser, &field_path, ctx)?;
                    py_dict_set_item(&dict, field.name.as_ptr(), val)?;
                    seen[idx >> 6] |= 1u64 << (idx & 63);
                }
                Route::Skip => parser.skip_value()?,
            }
            route = resolve_route(&self.format_routing, parser.next_key()?);
        }
        for (idx, field) in self.fields.iter().enumerate() {
            if seen[idx >> 6] & (1u64 << (idx & 63)) == 0 {
                // Missing field: same default / missing-required error as the
                // dict-path (get_default pushes dict_key_rs onto the base path).
                let val = field.get_default(py, instance_path)?;
                py_dict_set_item(&dict, field.name.as_ptr(), val)?;
            }
        }
        Ok(dict.into_any())
    }

    fn as_container_encoder(&self) -> Option<&dyn ContainerEncoder> {
        Some(self)
    }
}

impl ContainerEncoder for TypedDictEncoder {
    fn get_fields(&self) -> QueryFields<'_> {
        get_fields_query(&self.fields)
    }
}

#[derive(Debug, Clone)]
pub struct UUIDEncoder {
    pub(crate) uuid_cls: Py<PyAny>,
}

impl Encoder for UUIDEncoder {
    #[inline]
    fn dump<'a>(&self, value: &Bound<'a, PyAny>, _ctx: &Context) -> SerdeResult<Bound<'a, PyAny>> {
        Ok(value.str()?.into_any())
    }

    #[inline]
    fn load<'a>(
        &self,
        value: &Bound<'a, PyAny>,
        instance_path: &InstancePath,
        _ctx: &Context,
    ) -> SerdeResult<Bound<'a, PyAny>> {
        if let Ok(val) = value.cast::<PyString>() {
            if Uuid::parse_str(val.to_str()?).is_ok() {
                if let Ok(result) = self.uuid_cls.bind(value.py()).call1((val,)) {
                    return Ok(result);
                }
            }
        }
        invalid_type!("uuid", value, instance_path)
    }

    #[inline]
    fn dump_format(
        &self,
        value: &Bound<'_, PyAny>,
        writer: &mut Writer,
        _ctx: &Context,
    ) -> SerdeResult<()> {
        writer.write_str(value.str()?.to_str()?);
        Ok(())
    }

    fn load_format<'py>(
        &self,
        py: Python<'py>,
        parser: &mut Parser<'_>,
        instance_path: &InstancePath,
        _ctx: &Context,
    ) -> SerdeResult<Bound<'py, PyAny>> {
        if parser.peek()? == Kind::Str {
            let s = parser.take_str_known()?;
            if Uuid::parse_str(s).is_ok() {
                if let Ok(result) = self.uuid_cls.bind(py).call1((s,)) {
                    return Ok(result);
                }
            }
            // Invalid UUID text (or the constructor rejected it) -> native error,
            // no Python materialization.
            return Err(wrong_type_err("uuid", s, instance_path));
        }
        let raw = parser.take_raw_value()?;
        let raw = String::from_utf8_lossy(raw);
        Err(wrong_type_err("uuid", &raw, instance_path))
    }
}

#[derive(Debug, Clone)]
pub struct EnumEncoder {
    pub(crate) enum_items: String,
    pub(crate) load_map: Py<PyDict>,
    pub(crate) dump_map: IntMap<usize, Py<PyAny>>,
}

impl Encoder for EnumEncoder {
    #[inline]
    fn dump<'a>(&self, value: &Bound<'a, PyAny>, _ctx: &Context) -> SerdeResult<Bound<'a, PyAny>> {
        let id = value.as_ptr() as *const _ as usize;
        if let Some(py_item) = self.dump_map.get(&id) {
            return Ok(py_item.bind(value.py()).clone());
        }
        invalid_enum_item!(&self.enum_items, value, &InstancePath::new())
    }

    #[inline]
    fn load<'a>(
        &self,
        value: &Bound<'a, PyAny>,
        instance_path: &InstancePath,
        ctx: &Context,
    ) -> SerdeResult<Bound<'a, PyAny>> {
        match self.load_map.bind(value.py()).get_item(value) {
            Ok(Some(val)) => Ok(val),
            _ if ctx.try_cast_from_string => {
                if let Ok(Some(val)) = self.load_map.bind(value.py()).get_item((&value, false)) {
                    return Ok(val);
                }
                invalid_enum_item!(&self.enum_items, value, instance_path)
            }
            _ => invalid_enum_item!(&self.enum_items, value, instance_path),
        }
    }

    // Same lookup as `dump`, but streams the resolved scalar value directly
    // instead of returning it for the bridge to re-sniff via `write_any`.
    fn dump_format(
        &self,
        value: &Bound<'_, PyAny>,
        writer: &mut Writer,
        ctx: &Context,
    ) -> SerdeResult<()> {
        let id = value.as_ptr() as *const _ as usize;
        if let Some(py_item) = self.dump_map.get(&id) {
            return write_scalar_item(py_item.bind(value.py()), writer, ctx);
        }
        invalid_enum_item!(&self.enum_items, value, &InstancePath::new())
    }

    // Reads the scalar directly from the parser for the common str/int members,
    // then delegates the map lookup + error handling to `self.load` so the
    // miss/error behavior stays byte-identical to the object path. Non-scalar
    // (or float-shaped) tokens fall back through `parse_any`/`self.load`.
    fn load_format<'py>(
        &self,
        py: Python<'py>,
        parser: &mut Parser<'_>,
        instance_path: &InstancePath,
        ctx: &Context,
    ) -> SerdeResult<Bound<'py, PyAny>> {
        match parser.peek()? {
            Kind::Str => {
                let key = PyString::new(py, parser.take_str_known()?).into_any();
                self.load(&key, instance_path, ctx)
            }
            Kind::Num => match parser.take_int_known() {
                Ok(ParsedInt::I64(v)) => {
                    let key = v.into_bound_py_any(py)?;
                    self.load(&key, instance_path, ctx)
                }
                Ok(ParsedInt::Big(big)) => {
                    let key = big.into_bound_py_any(py)?;
                    self.load(&key, instance_path, ctx)
                }
                // Float-shaped token: the cursor is unmoved, so re-read the raw
                // number and build a float, matching `parse_any` -> `self.load`
                // which yields `invalid_enum_item` for a non-member.
                Err(_) => {
                    let raw = parser.take_number_str_known()?;
                    let v: f64 = raw.parse().map_err(|_| {
                        SerdeError::Py(ValidationError::new_err(format!("invalid number: {raw}")))
                    })?;
                    let key = PyFloat::new(py, v).into_any();
                    self.load(&key, instance_path, ctx)
                }
            },
            _ => {
                let raw = parser.take_raw_value()?;
                let raw = String::from_utf8_lossy(raw);
                Err(wrong_enum_err(&self.enum_items, &raw, instance_path))
            }
        }
    }
}

#[derive(Debug, Clone)]
pub struct LiteralEncoder {
    pub(crate) enum_items: String,
    pub(crate) load_map: Py<PyDict>,
    pub(crate) dump_map: Py<PyDict>,
}

impl Encoder for LiteralEncoder {
    #[inline]
    fn dump<'a>(&self, value: &Bound<'a, PyAny>, _ctx: &Context) -> SerdeResult<Bound<'a, PyAny>> {
        if let Ok(Some(py_item)) = self.dump_map.bind(value.py()).get_item(value) {
            return Ok(py_item);
        }
        invalid_enum_item!(&self.enum_items, value, &InstancePath::new())
    }

    #[inline]
    fn load<'a>(
        &self,
        value: &Bound<'a, PyAny>,
        instance_path: &InstancePath,
        ctx: &Context,
    ) -> SerdeResult<Bound<'a, PyAny>> {
        match self.load_map.bind(value.py()).get_item(value) {
            Ok(Some(val)) => Ok(val),
            _ if ctx.try_cast_from_string => {
                if let Ok(Some(val)) = self.load_map.bind(value.py()).get_item((&value, false)) {
                    return Ok(val);
                }
                invalid_enum_item!(&self.enum_items, value, instance_path)
            }
            _ => invalid_enum_item!(&self.enum_items, value, instance_path),
        }
    }

    // Same lookup as `dump`, but streams the resolved scalar value directly
    // instead of returning it for the bridge to re-sniff via `write_any`.
    fn dump_format(
        &self,
        value: &Bound<'_, PyAny>,
        writer: &mut Writer,
        ctx: &Context,
    ) -> SerdeResult<()> {
        if let Ok(Some(py_item)) = self.dump_map.bind(value.py()).get_item(value) {
            return write_scalar_item(&py_item, writer, ctx);
        }
        invalid_enum_item!(&self.enum_items, value, &InstancePath::new())
    }

    // Reads the scalar directly from the parser for the common str/int members,
    // then delegates the map lookup + error handling to `self.load` so the
    // miss/error behavior stays byte-identical to the object path. Non-scalar
    // (or float-shaped) tokens fall back through `parse_any`/`self.load`.
    fn load_format<'py>(
        &self,
        py: Python<'py>,
        parser: &mut Parser<'_>,
        instance_path: &InstancePath,
        ctx: &Context,
    ) -> SerdeResult<Bound<'py, PyAny>> {
        match parser.peek()? {
            Kind::Str => {
                let key = PyString::new(py, parser.take_str_known()?).into_any();
                self.load(&key, instance_path, ctx)
            }
            Kind::Num => match parser.take_int_known() {
                Ok(ParsedInt::I64(v)) => {
                    let key = v.into_bound_py_any(py)?;
                    self.load(&key, instance_path, ctx)
                }
                Ok(ParsedInt::Big(big)) => {
                    let key = big.into_bound_py_any(py)?;
                    self.load(&key, instance_path, ctx)
                }
                // Float-shaped token: the cursor is unmoved, so re-read the raw
                // number and build a float, matching `parse_any` -> `self.load`
                // which yields `invalid_enum_item` for a non-member.
                Err(_) => {
                    let raw = parser.take_number_str_known()?;
                    let v: f64 = raw.parse().map_err(|_| {
                        SerdeError::Py(ValidationError::new_err(format!("invalid number: {raw}")))
                    })?;
                    let key = PyFloat::new(py, v).into_any();
                    self.load(&key, instance_path, ctx)
                }
            },
            _ => {
                let raw = parser.take_raw_value()?;
                let raw = String::from_utf8_lossy(raw);
                Err(wrong_enum_err(&self.enum_items, &raw, instance_path))
            }
        }
    }
}

#[derive(Debug, Clone)]
pub struct OptionalEncoder {
    pub(crate) encoder: Box<TEncoder>,
}

impl Encoder for OptionalEncoder {
    #[inline]
    fn dump<'a>(&self, value: &Bound<'a, PyAny>, ctx: &Context) -> SerdeResult<Bound<'a, PyAny>> {
        if value.is_none() {
            Ok(value.clone())
        } else {
            self.encoder.dump(value, ctx)
        }
    }

    #[inline]
    fn load<'a>(
        &self,
        value: &Bound<'a, PyAny>,
        instance_path: &InstancePath,
        ctx: &Context,
    ) -> SerdeResult<Bound<'a, PyAny>> {
        if value.is_none() {
            Ok(value.clone())
        } else {
            self.encoder.load(value, instance_path, ctx)
        }
    }

    #[inline]
    fn dump_format(
        &self,
        value: &Bound<'_, PyAny>,
        writer: &mut Writer,
        ctx: &Context,
    ) -> SerdeResult<()> {
        if value.is_none() {
            writer.write_null();
            Ok(())
        } else {
            self.encoder.dump_format(value, writer, ctx)
        }
    }

    #[inline]
    fn load_format<'py>(
        &self,
        py: Python<'py>,
        parser: &mut Parser<'_>,
        instance_path: &InstancePath,
        ctx: &Context,
    ) -> SerdeResult<Bound<'py, PyAny>> {
        if parser.peek()? == Kind::Null {
            parser.take_null_known()?;
            Ok(py.None().into_bound(py))
        } else {
            self.encoder.load_format(py, parser, instance_path, ctx)
        }
    }

    fn is_sequence(&self) -> bool {
        self.encoder.is_sequence()
    }
}

#[derive(Debug, Clone)]
pub struct TupleEncoder {
    pub(crate) encoders: Vec<Box<TEncoder>>,
}

impl Encoder for TupleEncoder {
    #[inline]
    fn dump<'a>(&self, value: &Bound<'a, PyAny>, ctx: &Context) -> SerdeResult<Bound<'a, PyAny>> {
        if let Ok(seq) = value.cast::<PySequence>() {
            let seq_len = seq.len()?;
            check_sequence_size(seq, seq_len, self.encoders.len(), None)?;
            let result = create_py_list(value.py(), seq_len)?;
            for index in 0..seq_len {
                let item = seq.get_item(index)?;
                let val = self.encoders[index].dump(&item, ctx)?;
                py_list_set_item(&result, index, val);
            }

            Ok(result.into_any())
        } else {
            invalid_type_dump!("sequence", value)
        }
    }

    #[inline]
    fn load<'a>(
        &self,
        value: &Bound<'a, PyAny>,
        instance_path: &InstancePath,
        ctx: &Context,
    ) -> SerdeResult<Bound<'a, PyAny>> {
        // Check sequence is not str
        if let Ok(seq) = value.cast::<PySequence>() {
            if value.is_instance_of::<PyString>() {
                invalid_type!("sequence", value, instance_path);
            }
            let seq_len = seq.len()?;
            check_sequence_size(seq, seq_len, self.encoders.len(), Some(instance_path))?;
            let result = create_py_tuple(value.py(), seq_len)?;
            for index in 0..seq_len {
                let item = seq.get_item(index)?;
                let instance_path = instance_path.push(index);
                let val = self.encoders[index].load(&item, &instance_path, ctx)?;
                py_tuple_set_item(&result, index, val);
            }
            Ok(result.into_any())
        } else {
            invalid_type!("sequence", value, instance_path)
        }
    }

    #[inline]
    fn dump_format(
        &self,
        value: &Bound<'_, PyAny>,
        writer: &mut Writer,
        ctx: &Context,
    ) -> SerdeResult<()> {
        let _guard = ctx.enter_depth()?;
        if let Ok(seq) = value.cast::<PySequence>() {
            let seq_len = seq.len()?;
            check_sequence_size(seq, seq_len, self.encoders.len(), None)?;
            writer.begin_array();
            for index in 0..seq_len {
                writer.array_item();
                let item = seq.get_item(index)?;
                self.encoders[index].dump_format(&item, writer, ctx)?;
            }
            writer.end_array();
            Ok(())
        } else {
            invalid_type_dump!("sequence", value)
        }
    }

    fn load_format<'py>(
        &self,
        py: Python<'py>,
        parser: &mut Parser<'_>,
        instance_path: &InstancePath,
        ctx: &Context,
    ) -> SerdeResult<Bound<'py, PyAny>> {
        let _guard = ctx.enter_depth()?;
        if parser.peek()? != Kind::Array {
            let raw = parser.take_raw_value()?;
            let raw = String::from_utf8_lossy(raw);
            return Err(wrong_type_err("sequence", &raw, instance_path));
        }
        let mut items: Vec<Bound<'py, PyAny>> = Vec::new();
        if parser.enter_array_known()? {
            loop {
                let idx = items.len();
                if idx < self.encoders.len() {
                    let item_path = instance_path.push(idx);
                    items.push(self.encoders[idx].load_format(py, parser, &item_path, ctx)?);
                } else {
                    // More items than expected encoders: consume them
                    // generically so the final count still reflects what the
                    // input actually contains, giving the same "has more
                    // than N items" error check_sequence_size would raise.
                    items.push(parse_any(py, parser, ctx)?);
                }
                if !parser.next_array_item()? {
                    break;
                }
            }
        }
        // Length can only be known once the closing bracket is seen, so a
        // type error on one of the first `encoders.len()` items surfaces
        // before a would-be length error here (unlike the dict path, which
        // checks length against the raw input up front). The list built from
        // already-converted items is used for the length-error message
        // (dict path shows the raw input instead).
        let list = PyList::new(py, items)?;
        let seq = list.cast::<PySequence>().map_err(PyErr::from)?;
        check_sequence_size(seq, list.len(), self.encoders.len(), Some(instance_path))?;
        let result = create_py_tuple(py, list.len())?;
        for (index, item) in list.iter().enumerate() {
            py_tuple_set_item(&result, index, item);
        }
        Ok(result.into_any())
    }

    fn is_sequence(&self) -> bool {
        true
    }
}

#[derive(Debug, Clone)]
pub struct UnionEncoder {
    pub(crate) encoders: Vec<Box<TEncoder>>,
    pub(crate) repr: String,
}

impl Encoder for UnionEncoder {
    #[inline]
    fn dump<'a>(&self, value: &Bound<'a, PyAny>, ctx: &Context) -> SerdeResult<Bound<'a, PyAny>> {
        for encoder in &self.encoders {
            match encoder.dump(value, ctx) {
                Ok(v) => return Ok(v),
                Err(SerdeError::Schema(_)) => continue,
                Err(e @ SerdeError::Py(_)) => return Err(e),
            }
        }
        Err(invalid_type_dump_err(&self.repr, value))
    }

    #[inline]
    fn load<'a>(
        &self,
        value: &Bound<'a, PyAny>,
        instance_path: &InstancePath,
        ctx: &Context,
    ) -> SerdeResult<Bound<'a, PyAny>> {
        for encoder in &self.encoders {
            match encoder.load(value, instance_path, ctx) {
                Ok(v) => return Ok(v),
                Err(SerdeError::Schema(_)) => continue,
                Err(e @ SerdeError::Py(_)) => return Err(e),
            }
        }
        Err(invalid_type_err(&self.repr, value, instance_path))
    }

    // dump_format probes each member's *validating* dump_format into a throwaway
    // writer of the same format; the first that succeeds is spliced into the real
    // writer as one complete value. The bridge default is unusable here: it goes
    // through self.dump, whose member loop calls each scalar encoder's dump, and
    // those do NOT type-validate (they clone the value), so `int | Entity` would
    // "succeed" on the int member and then fail in write_any.
    fn dump_format(
        &self,
        value: &Bound<'_, PyAny>,
        writer: &mut Writer,
        ctx: &Context,
    ) -> SerdeResult<()> {
        for encoder in &self.encoders {
            // A member that fails mid-write only corrupts its throwaway probe,
            // never the real writer.
            let mut probe = writer.new_probe();
            match encoder.dump_format(value, &mut probe, ctx) {
                Ok(()) => {
                    writer.write_raw_value(probe.as_bytes());
                    return Ok(());
                }
                Err(SerdeError::Schema(_)) => continue,
                Err(e @ SerdeError::Py(_)) => return Err(e),
            }
        }
        Err(invalid_type_dump_err(&self.repr, value))
    }

    fn load_format<'py>(
        &self,
        py: Python<'py>,
        parser: &mut Parser<'_>,
        instance_path: &InstancePath,
        ctx: &Context,
    ) -> SerdeResult<Bound<'py, PyAny>> {
        // One mechanism for every kind: capture the raw span, try each member on a
        // fresh sub-parser. A member that partially consumes then fails cannot
        // corrupt the main cursor (take_raw_value already advanced it past the value).
        let span = parser.take_raw_value()?;
        for encoder in &self.encoders {
            let mut sub = parser.sub_parser(span);
            match encoder.load_format(py, &mut sub, instance_path, ctx) {
                Ok(v) => return Ok(v),
                Err(SerdeError::Schema(_)) => continue,
                Err(e @ SerdeError::Py(_)) => return Err(e),
            }
        }
        // No member matched: native Schema error, no materialization.
        let raw = String::from_utf8_lossy(span);
        Err(wrong_type_err(&self.repr, &raw, instance_path))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct DiscriminatorKey(String);

impl TryFrom<&Bound<'_, PyAny>> for DiscriminatorKey {
    type Error = ();

    fn try_from(value: &Bound<'_, PyAny>) -> Result<Self, Self::Error> {
        if let Ok(val) = value.cast::<PyString>() {
            Ok(DiscriminatorKey(val.to_string()))
        } else if let Ok(value) = value.getattr(intern!(value.py(), "value")) {
            DiscriminatorKey::try_from(&value)
        } else {
            Err(())
        }
    }
}

impl fmt::Display for DiscriminatorKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Debug, Clone)]
pub struct DiscriminatedUnionEncoder {
    pub(crate) encoders: HashMap<DiscriminatorKey, Box<TEncoder>>,
    pub(crate) dump_discriminator: Py<PyString>,
    pub(crate) load_discriminator: Py<PyString>,
    pub(crate) load_discriminator_rs: String,
    pub(crate) keys: Vec<DiscriminatorKey>,
}

impl Encoder for DiscriminatedUnionEncoder {
    #[inline]
    fn dump<'a>(&self, value: &Bound<'a, PyAny>, ctx: &Context) -> SerdeResult<Bound<'a, PyAny>> {
        let key = match value.getattr(&self.dump_discriminator) {
            Ok(val) => val,
            Err(_) => {
                return Err(missing_required_property(
                    self.dump_discriminator.bind(value.py()).str()?.to_str()?,
                    &InstancePath::new(),
                ));
            }
        };

        let key = DiscriminatorKey::try_from(&key)
            .map_err(|_| no_encoder_for_discriminator(&key, &self.keys, &InstancePath::new()))?;

        let encoder = self.encoders.get(&key).ok_or_else(|| {
            let instance_path = InstancePath::new();
            no_encoder_for_discriminator(&key, &self.keys, &instance_path)
        })?;
        encoder.dump(value, ctx)
    }

    #[inline]
    fn load<'a>(
        &self,
        value: &Bound<'a, PyAny>,
        instance_path: &InstancePath,
        ctx: &Context,
    ) -> SerdeResult<Bound<'a, PyAny>> {
        if let Ok(val) = value.cast::<PyDict>() {
            let key = match val.get_item(&self.load_discriminator) {
                Ok(Some(k)) => k,
                _ => {
                    return Err(missing_required_property(
                        &self.load_discriminator_rs,
                        instance_path,
                    ));
                }
            };

            let key = DiscriminatorKey::try_from(&key).map_err(|_| {
                no_encoder_for_discriminator(&key.to_string(), &self.keys, instance_path)
            })?;

            let encoder = self.encoders.get(&key).ok_or_else(|| {
                let instance_path = instance_path.push(self.load_discriminator_rs.as_str());
                no_encoder_for_discriminator(&key, &self.keys, &instance_path)
            })?;
            encoder.load(value, instance_path, ctx)
        } else {
            invalid_type!("dict", value, instance_path)
        }
    }

    // dump_format stays on the bridge default: it materializes via self.dump
    // (which selects the variant by discriminator) and writes the result.

    fn load_format<'py>(
        &self,
        py: Python<'py>,
        parser: &mut Parser<'_>,
        instance_path: &InstancePath,
        ctx: &Context,
    ) -> SerdeResult<Bound<'py, PyAny>> {
        if parser.peek()? != Kind::Map {
            let raw = parser.take_raw_value()?;
            let raw = String::from_utf8_lossy(raw);
            return Err(wrong_type_err("dict", &raw, instance_path));
        }
        let span = parser.take_raw_value()?;
        // Scan forward on a throwaway sub-parser to find the discriminator value,
        // regardless of key order.
        let mut tag: Option<String> = None;
        {
            let mut scan = parser.sub_parser(span);
            let mut key = scan.enter_map()?.map(str::to_owned);
            while let Some(k) = key {
                if k == self.load_discriminator_rs {
                    if scan.peek()? == Kind::Str {
                        tag = Some(scan.take_str_known()?.to_owned());
                    }
                    break;
                }
                scan.skip_value()?;
                key = scan.next_key()?.map(str::to_owned);
            }
        }
        let Some(tag) = tag else {
            // Missing/non-string discriminator: re-run the object path for the exact
            // error (missing_required_property, or the non-string discriminator error).
            let mut sub = parser.sub_parser(span);
            let value = parse_any(py, &mut sub, ctx)?;
            return self.load(&value, instance_path, ctx);
        };
        // Select the encoder by tag; unknown tag -> same Schema error (message and
        // instance_path) as the object path.
        match self.encoders.get(&DiscriminatorKey(tag.clone())) {
            Some(encoder) => {
                let mut sub = parser.sub_parser(span);
                encoder.load_format(py, &mut sub, instance_path, ctx)
            }
            None => {
                let instance_path = instance_path.push(self.load_discriminator_rs.as_str());
                Err(no_encoder_for_discriminator(
                    &tag,
                    &self.keys,
                    &instance_path,
                ))
            }
        }
    }
}

#[derive(Debug, Clone)]
pub struct TimeEncoder {}

impl Encoder for TimeEncoder {
    #[inline]
    fn dump<'a>(&self, value: &Bound<'a, PyAny>, _ctx: &Context) -> SerdeResult<Bound<'a, PyAny>> {
        let py_time = value
            .cast::<PyTime>()
            .map_err(|_| invalid_type_dump_err("time", value))?;
        let result = dump_time(py_time)?;
        Ok(result.into_bound_py_any(value.py())?)
    }

    #[inline]
    fn load<'a>(
        &self,
        value: &Bound<'a, PyAny>,
        instance_path: &InstancePath,
        _ctx: &Context,
    ) -> SerdeResult<Bound<'a, PyAny>> {
        if let Ok(val) = value.cast::<PyString>() {
            if let Ok(result) = parse_time(value.py(), val.to_str()?) {
                return Ok(result.into_any());
            }
        }
        invalid_type!("time", value, instance_path)
    }

    #[inline]
    fn dump_format(
        &self,
        value: &Bound<'_, PyAny>,
        writer: &mut Writer,
        _ctx: &Context,
    ) -> SerdeResult<()> {
        let py_time = value
            .cast::<PyTime>()
            .map_err(|_| invalid_type_dump_err("time", value))?;
        let result = dump_time(py_time)?;
        writer.write_str(&result);
        Ok(())
    }

    fn load_format<'py>(
        &self,
        py: Python<'py>,
        parser: &mut Parser<'_>,
        instance_path: &InstancePath,
        _ctx: &Context,
    ) -> SerdeResult<Bound<'py, PyAny>> {
        if parser.peek()? == Kind::Str {
            let s = parser.take_str_known()?;
            if let Ok(result) = parse_time(py, s) {
                return Ok(result.into_any());
            }
            // Invalid time text -> native error, no Python materialization.
            return Err(wrong_type_err("time", s, instance_path));
        }
        let raw = parser.take_raw_value()?;
        let raw = String::from_utf8_lossy(raw);
        Err(wrong_type_err("time", &raw, instance_path))
    }
}

#[derive(Debug, Clone)]
pub struct DateTimeEncoder {
    pub(crate) naive_datetime_to_utc: bool,
}

impl Encoder for DateTimeEncoder {
    #[inline]
    fn dump<'a>(&self, value: &Bound<'a, PyAny>, _ctx: &Context) -> SerdeResult<Bound<'a, PyAny>> {
        let py_datetime = value
            .cast::<PyDateTime>()
            .map_err(|_| invalid_type_dump_err("datetime", value))?;
        let result = dump_datetime(py_datetime, self.naive_datetime_to_utc)?;
        Ok(result.into_bound_py_any(value.py())?)
    }

    #[inline]
    fn load<'a>(
        &self,
        value: &Bound<'a, PyAny>,
        instance_path: &InstancePath,
        _ctx: &Context,
    ) -> SerdeResult<Bound<'a, PyAny>> {
        if let Ok(val) = value.cast::<PyString>() {
            if let Ok(result) = parse_datetime(value.py(), val.to_str()?) {
                return Ok(result.into_any());
            }
        }
        invalid_type!("datetime", value, instance_path)
    }

    #[inline]
    fn dump_format(
        &self,
        value: &Bound<'_, PyAny>,
        writer: &mut Writer,
        _ctx: &Context,
    ) -> SerdeResult<()> {
        let py_datetime = value
            .cast::<PyDateTime>()
            .map_err(|_| invalid_type_dump_err("datetime", value))?;
        let result = dump_datetime(py_datetime, self.naive_datetime_to_utc)?;
        writer.write_str(&result);
        Ok(())
    }

    fn load_format<'py>(
        &self,
        py: Python<'py>,
        parser: &mut Parser<'_>,
        instance_path: &InstancePath,
        _ctx: &Context,
    ) -> SerdeResult<Bound<'py, PyAny>> {
        if parser.peek()? == Kind::Str {
            let s = parser.take_str_known()?;
            if let Ok(result) = parse_datetime(py, s) {
                return Ok(result.into_any());
            }
            // Invalid datetime text -> native error, no Python materialization.
            return Err(wrong_type_err("datetime", s, instance_path));
        }
        let raw = parser.take_raw_value()?;
        let raw = String::from_utf8_lossy(raw);
        Err(wrong_type_err("datetime", &raw, instance_path))
    }
}

#[derive(Debug, Clone)]
pub struct DateEncoder {}

impl Encoder for DateEncoder {
    #[inline]
    fn dump<'a>(&self, value: &Bound<'a, PyAny>, _ctx: &Context) -> SerdeResult<Bound<'a, PyAny>> {
        let py_date = value
            .cast::<PyDate>()
            .map_err(|_| invalid_type_dump_err("date", value))?;
        let result = dump_date(py_date);
        Ok(result.into_bound_py_any(value.py())?)
    }

    #[inline]
    fn load<'a>(
        &self,
        value: &Bound<'a, PyAny>,
        instance_path: &InstancePath,
        _ctx: &Context,
    ) -> SerdeResult<Bound<'a, PyAny>> {
        if let Ok(val) = value.cast::<PyString>() {
            if let Ok(result) = parse_date(value.py(), val.to_str()?) {
                return Ok(result.into_any());
            }
        }
        invalid_type!("date", value, instance_path)
    }

    #[inline]
    fn dump_format(
        &self,
        value: &Bound<'_, PyAny>,
        writer: &mut Writer,
        _ctx: &Context,
    ) -> SerdeResult<()> {
        let py_date = value
            .cast::<PyDate>()
            .map_err(|_| invalid_type_dump_err("date", value))?;
        let result = dump_date(py_date);
        writer.write_str(&result);
        Ok(())
    }

    fn load_format<'py>(
        &self,
        py: Python<'py>,
        parser: &mut Parser<'_>,
        instance_path: &InstancePath,
        _ctx: &Context,
    ) -> SerdeResult<Bound<'py, PyAny>> {
        if parser.peek()? == Kind::Str {
            let s = parser.take_str_known()?;
            if let Ok(result) = parse_date(py, s) {
                return Ok(result.into_any());
            }
            // Invalid date text -> native error, no Python materialization.
            return Err(wrong_type_err("date", s, instance_path));
        }
        let raw = parser.take_raw_value()?;
        let raw = String::from_utf8_lossy(raw);
        Err(wrong_type_err("date", &raw, instance_path))
    }
}

/// Placeholder for a recursive encoder.
///
/// During `get_encoder` we eagerly build encoders for nested types; when a
/// type references itself we hand out a `LazyEncoder` and back-fill the inner
/// `Arc<dyn Encoder>` after the surrounding encoder is built. Dump/load is a
/// single dynamic dispatch through the trait object, no per-variant match.
/// `OnceLock` makes the back-fill thread-safe under free-threaded Python:
/// the inner slot is written exactly once during `Serializer::new` and read
/// concurrently from any number of threads afterwards.
#[derive(Debug, Clone)]
pub struct LazyEncoder {
    pub(crate) inner: Arc<OnceLock<Arc<TEncoder>>>,
}

impl Encoder for LazyEncoder {
    #[inline]
    fn dump<'a>(&self, value: &Bound<'a, PyAny>, ctx: &Context) -> SerdeResult<Bound<'a, PyAny>> {
        let _guard = ctx.enter_depth()?;
        match self.inner.get() {
            Some(encoder) => encoder.dump(value, ctx),
            None => Err(SerdeError::Py(PyRuntimeError::new_err(
                "[RUST] Invalid recursive encoder".to_string(),
            ))),
        }
    }

    #[inline]
    fn load<'a>(
        &self,
        value: &Bound<'a, PyAny>,
        instance_path: &InstancePath,
        ctx: &Context,
    ) -> SerdeResult<Bound<'a, PyAny>> {
        let _guard = ctx.enter_depth()?;
        match self.inner.get() {
            Some(encoder) => encoder.load(value, instance_path, ctx),
            None => Err(SerdeError::Py(PyRuntimeError::new_err(
                "[RUST] Invalid recursive encoder".to_string(),
            ))),
        }
    }

    #[inline]
    fn dump_format(
        &self,
        value: &Bound<'_, PyAny>,
        writer: &mut Writer,
        ctx: &Context,
    ) -> SerdeResult<()> {
        let _guard = ctx.enter_depth()?;
        match self.inner.get() {
            Some(encoder) => encoder.dump_format(value, writer, ctx),
            None => Err(SerdeError::Py(PyRuntimeError::new_err(
                "[RUST] Invalid recursive encoder".to_string(),
            ))),
        }
    }

    #[inline]
    fn load_format<'py>(
        &self,
        py: Python<'py>,
        parser: &mut Parser<'_>,
        instance_path: &InstancePath,
        ctx: &Context,
    ) -> SerdeResult<Bound<'py, PyAny>> {
        let _guard = ctx.enter_depth()?;
        match self.inner.get() {
            Some(encoder) => encoder.load_format(py, parser, instance_path, ctx),
            None => Err(SerdeError::Py(PyRuntimeError::new_err(
                "[RUST] Invalid recursive encoder".to_string(),
            ))),
        }
    }
}

#[derive(Debug, Clone)]
pub struct CustomEncoder {
    pub(crate) inner: Box<TEncoder>,
    pub(crate) dump: Option<Py<PyAny>>,
    pub(crate) load: Option<Py<PyAny>>,
}

impl Encoder for CustomEncoder {
    #[inline]
    fn dump<'a>(&self, value: &Bound<'a, PyAny>, ctx: &Context) -> SerdeResult<Bound<'a, PyAny>> {
        match self.dump {
            Some(ref dump) => dump
                .bind(value.py())
                .call1((value,))
                .map_err(|err| SerdeError::from_user_callback(err, &InstancePath::new())),
            None => self.inner.dump(value, ctx),
        }
    }

    #[inline]
    fn load<'a>(
        &self,
        value: &Bound<'a, PyAny>,
        instance_path: &InstancePath,
        ctx: &Context,
    ) -> SerdeResult<Bound<'a, PyAny>> {
        match self.load {
            Some(ref load) => load
                .bind(value.py())
                .call1((value,))
                .map_err(|err| SerdeError::from_user_callback(err, instance_path)),
            None => self.inner.load(value, instance_path, ctx),
        }
    }

    // With a user callback we must materialize (run the callback via dump/load,
    // then bridge the plain object); without one, delegate straight to inner's
    // format methods to keep any direct optimization intact.
    fn dump_format(
        &self,
        value: &Bound<'_, PyAny>,
        writer: &mut Writer,
        ctx: &Context,
    ) -> SerdeResult<()> {
        match self.dump {
            Some(_) => {
                let dumped = self.dump(value, ctx)?;
                write_any(&dumped, writer, ctx)
            }
            None => self.inner.dump_format(value, writer, ctx),
        }
    }

    fn load_format<'py>(
        &self,
        py: Python<'py>,
        parser: &mut Parser<'_>,
        instance_path: &InstancePath,
        ctx: &Context,
    ) -> SerdeResult<Bound<'py, PyAny>> {
        match self.load {
            Some(_) => {
                let value = parse_any(py, parser, ctx)?;
                self.load(&value, instance_path, ctx)
            }
            None => self.inner.load_format(py, parser, instance_path, ctx),
        }
    }

    fn is_sequence(&self) -> bool {
        self.inner.is_sequence()
    }
}

#[derive(Debug, Clone)]
pub struct CustomTypeEncoder {
    pub(crate) dump: Py<PyAny>,
    pub(crate) load: Py<PyAny>,
}

impl Encoder for CustomTypeEncoder {
    #[inline]
    fn dump<'a>(&self, value: &Bound<'a, PyAny>, _ctx: &Context) -> SerdeResult<Bound<'a, PyAny>> {
        self.dump
            .bind(value.py())
            .call1((value,))
            .map_err(|err| SerdeError::from_user_callback(err, &InstancePath::new()))
    }

    #[inline]
    fn load<'a>(
        &self,
        value: &Bound<'a, PyAny>,
        instance_path: &InstancePath,
        _ctx: &Context,
    ) -> SerdeResult<Bound<'a, PyAny>> {
        self.load
            .bind(value.py())
            .call1((value,))
            .map_err(|err| SerdeError::from_user_callback(err, instance_path))
    }
}
