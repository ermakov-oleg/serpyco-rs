use std::collections::HashMap;

use rustc_hash::FxHashMap;
use smallvec::{smallvec, SmallVec};
use std::cmp::Ordering;
use std::fmt;
use std::fmt::Debug;
use std::sync::{Arc, OnceLock};

use dyn_clone::{clone_trait_object, DynClone};
use nohash_hasher::IntMap;
use pyo3::exceptions::{PyAttributeError, PyRuntimeError};
use pyo3::types::{
    PyBool, PyBytes, PyDate, PyDateTime, PyDict, PyFloat, PyInt, PyList, PySequence, PySet,
    PyString, PyTime, PyType,
};
use pyo3::{intern, Bound, Py, PyAny, PyResult};
use pyo3::{prelude::*, IntoPyObjectExt};
use uuid::Uuid;

use crate::errors::{ToPyErr, ValidationError};
use crate::format::bridge::{
    invalid_number_err, parse_any, write_any, write_py_float, write_py_int, wrong_enum_at_cursor,
    wrong_type_at_cursor, wrong_type_err,
};
use crate::format::{EncodedKey, Kind, ParsedInt, ParsedNumber, Parser, Writer};
use crate::python::{
    create_instance, create_py_dict_known_size, create_py_list, create_py_string, create_py_tuple,
    dump_date, dump_datetime, dump_time, generic_set_attr, parse_date, parse_datetime, parse_time,
    py_dict_set_item, py_list_get_item, py_list_set_item, py_tuple_set_item, set_attr_unchecked,
};
use crate::python::{DecimalTypeInfo, FloatTypeInfo, IntegerTypeInfo, StringTypeInfo};
use crate::serde_error::{Message, SchemaError, SerdeError, SerdeResult};
use crate::validator::validators::{
    check_bounds, check_length, check_sequence_bounds, check_sequence_size, invalid_enum_item,
    invalid_type, invalid_type_dump, invalid_type_dump_err, invalid_type_dump_err_with_cause,
    invalid_type_err, missing_required_property, no_encoder_for_discriminator, sequence_size_err,
    sequence_size_ordering, str_as_bool,
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

    /// Default: bridges through `dump` + `write_any`. Override only where a
    /// direct per-field streaming path is faster; the bridge error is already
    /// correct for pass-throughs, runtime redirects, and wire-less types.
    fn dump_format(
        &self,
        value: &Bound<'_, PyAny>,
        writer: &mut Writer,
        ctx: &Context,
    ) -> SerdeResult<()> {
        let dumped = self.dump(value, ctx)?;
        write_any(&dumped, writer, ctx)
    }

    /// Default: bridges through `parse_any` + `load`; see `dump_format`.
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

    /// Must stay conservative: returning `false` for a kind the encoder would
    /// accept turns a valid document into an error. Default accepts everything.
    fn accepts_kind(&self, _kind: Kind) -> bool {
        true
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
        Err(wrong_type_at_cursor(parser, "None", instance_path))
    }

    #[inline]
    fn accepts_kind(&self, kind: Kind) -> bool {
        matches!(kind, Kind::Null)
    }
}

#[derive(Debug, Clone)]
pub struct NeverEncoder;

impl Encoder for NeverEncoder {
    #[inline]
    fn dump<'a>(&self, value: &Bound<'a, PyAny>, _ctx: &Context) -> SerdeResult<Bound<'a, PyAny>> {
        invalid_type_dump!("Never", value)
    }

    #[inline]
    fn load<'a>(
        &self,
        value: &Bound<'a, PyAny>,
        instance_path: &InstancePath,
        _ctx: &Context,
    ) -> SerdeResult<Bound<'a, PyAny>> {
        invalid_type!("Never (no value allowed)", value, instance_path)
    }

    #[inline]
    fn dump_format(
        &self,
        value: &Bound<'_, PyAny>,
        _writer: &mut Writer,
        _ctx: &Context,
    ) -> SerdeResult<()> {
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
        Err(wrong_type_at_cursor(
            parser,
            "Never (no value allowed)",
            instance_path,
        ))
    }

    #[inline]
    fn accepts_kind(&self, _kind: Kind) -> bool {
        false
    }
}

/// Narrow to an `int` subclass that is not `bool`: `IntEnum`/`IntFlag` dump as
/// plain ints on the dict path, so the codec accepts them too; `bool` stays a
/// mismatch on a numeric field.
#[inline]
fn cast_int_subclass<'a, 'py>(value: &'a Bound<'py, PyAny>) -> Option<&'a Bound<'py, PyInt>> {
    if value.is_instance_of::<PyBool>() {
        return None;
    }
    value.cast::<PyInt>().ok()
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
            write_py_int(writer, v)?;
            return Ok(());
        }
        if let Some(v) = cast_int_subclass(value) {
            write_py_int(writer, v)?;
            return Ok(());
        }
        invalid_type_dump!("integer", value)
    }

    // jiter rejects a float-shaped token without advancing the cursor; on that
    // error re-read the raw number natively (no PyFloat materialization) to
    // classify DecodeError (malformed) vs SchemaValidationError (float-shaped).
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
                    // Unbounded: accept as-is (the i64 bounds-check would overflow).
                    if self.type_info.min.is_none() && self.type_info.max.is_none() {
                        return Ok(big.into_bound_py_any(py)?);
                    }
                    // Bounded: defer to `load` for the standard bounds check.
                    let materialized = big.into_bound_py_any(py)?;
                    return self.load(&materialized, instance_path, ctx);
                }
                Err(_) => {
                    // Cursor unmoved: re-read as raw text, whose grammar validation
                    // decides DecodeError vs SchemaValidationError below. The error
                    // renders that raw wire text rather than Python's float repr
                    // (`1e3` vs `1000.0`), diverging from the dict path — reviewer-
                    // waived, crit round 2 task 10; do not "fix" back.
                    let raw = parser.take_number_str_known()?;
                    return Err(wrong_type_err("integer", raw, instance_path));
                }
            }
        }
        Err(wrong_type_at_cursor(parser, "integer", instance_path))
    }

    #[inline]
    fn accepts_kind(&self, kind: Kind) -> bool {
        matches!(kind, Kind::Num)
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
            write_py_float(writer, v)?;
            return Ok(());
        }
        if let Ok(v) = value.cast_exact::<PyInt>() {
            write_py_int(writer, v)?;
            return Ok(());
        }
        // Every int but `bool`: a bool on a float field is a "number" mismatch
        // (the codec dump validates types; the dict path is lenient).
        if let Some(v) = cast_int_subclass(value) {
            write_py_int(writer, v)?;
            return Ok(());
        }
        invalid_type_dump!("number", value)
    }

    // Integer-shaped values defer to `load`, so a float field returns an int (not
    // 1.0) for `b'1'`, like the dict path; float-shaped values load directly.
    fn load_format<'py>(
        &self,
        py: Python<'py>,
        parser: &mut Parser<'_>,
        instance_path: &InstancePath,
        ctx: &Context,
    ) -> SerdeResult<Bound<'py, PyAny>> {
        if parser.peek()? == Kind::Num {
            match parser.take_number_known()? {
                ParsedNumber::Int(ParsedInt::I64(v)) => {
                    let materialized = v.into_bound_py_any(py)?;
                    return self.load(&materialized, instance_path, ctx);
                }
                ParsedNumber::Int(ParsedInt::Big(big)) => {
                    let materialized = big.into_bound_py_any(py)?;
                    return self.load(&materialized, instance_path, ctx);
                }
                ParsedNumber::F64(v) => {
                    check_bounds!(v, self.type_info, instance_path)?;
                    return Ok(PyFloat::new(py, v).into_any());
                }
            }
        }
        Err(wrong_type_at_cursor(parser, "number", instance_path))
    }

    #[inline]
    fn accepts_kind(&self, kind: Kind) -> bool {
        matches!(kind, Kind::Num)
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

    // Build Decimal from the raw JSON text (not an f64 round-trip) so precision
    // survives; the f64 parse is only for the bounds check.
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
                    // Unreachable: jiter only yields valid JSON numbers.
                    Err(_) => Err(invalid_number_err(raw)),
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
            _ => Err(wrong_type_at_cursor(parser, "decimal", instance_path)),
        }
    }

    #[inline]
    fn accepts_kind(&self, kind: Kind) -> bool {
        matches!(kind, Kind::Num | Kind::Str)
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
            let py_str = parser.take_pystring_known(py)?;
            check_length(
                &py_str,
                self.type_info.min_length,
                self.type_info.max_length,
                instance_path,
            )?;
            return Ok(py_str.into_any());
        }
        Err(wrong_type_at_cursor(parser, "string", instance_path))
    }

    #[inline]
    fn accepts_kind(&self, kind: Kind) -> bool {
        matches!(kind, Kind::Str)
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
        Err(wrong_type_at_cursor(parser, "boolean", instance_path))
    }

    #[inline]
    fn accepts_kind(&self, kind: Kind) -> bool {
        matches!(kind, Kind::Bool)
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

    fn dump_format(
        &self,
        value: &Bound<'_, PyAny>,
        writer: &mut Writer,
        _ctx: &Context,
    ) -> SerdeResult<()> {
        if let Ok(value) = value.cast::<PyBytes>() {
            writer
                .write_bytes(value.as_bytes())
                .map_err(|msg| SerdeError::Py(ValidationError::new_err(msg)))?;
            return Ok(());
        }
        invalid_type_dump!("bytes", value)
    }

    fn load_format<'py>(
        &self,
        py: Python<'py>,
        parser: &mut Parser<'_>,
        instance_path: &InstancePath,
        _ctx: &Context,
    ) -> SerdeResult<Bound<'py, PyAny>> {
        if parser.peek()? == Kind::Bytes {
            return Ok(PyBytes::new(py, parser.take_bytes_known()?).into_any());
        }
        Err(wrong_type_at_cursor(parser, "bytes", instance_path))
    }

    #[inline]
    fn accepts_kind(&self, kind: Kind) -> bool {
        matches!(kind, Kind::Bytes)
    }
}

/// Write a dict key as a map key, mirroring `bridge::write_any`'s key handling.
#[inline]
fn write_map_key(key: &Bound<'_, PyAny>, writer: &mut Writer) -> SerdeResult<()> {
    match key.cast::<PyString>() {
        Ok(s) => writer.map_key(s.to_str()?),
        Err(_) => writer.map_key(key.str()?.to_str()?),
    }
    Ok(())
}

/// omit_none for a fixed-key field: write key+value, then roll back if the value
/// encoded null — the streaming equivalent of the dict path's `is_none()`.
#[inline(always)]
fn dump_field_unless_null(
    writer: &mut Writer,
    key: &EncodedKey,
    encoder: &TEncoder,
    value: &Bound<'_, PyAny>,
    ctx: &Context,
) -> SerdeResult<()> {
    let cp = writer.checkpoint();
    writer.map_key_encoded(key);
    let value_start = writer.position();
    encoder.dump_format(value, writer, ctx)?;
    if writer.tail_is_null(value_start) {
        writer.rollback(cp);
    } else {
        writer.item_end();
    }
    Ok(())
}

/// Stream an enum/literal value directly by its concrete Python type; anything
/// exotic falls back to `write_any` for byte-identical output.
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
        write_py_int(writer, v)?;
        return Ok(());
    }
    if let Ok(v) = item.cast::<PyFloat>() {
        write_py_float(writer, v)?;
        return Ok(());
    }
    write_any(item, writer, ctx)
}

/// Read a scalar enum/literal member, then delegate the map lookup and error
/// handling to `load` for dict-path parity.
#[inline(always)]
fn load_enum_scalar<'py>(
    py: Python<'py>,
    parser: &mut Parser<'_>,
    instance_path: &InstancePath,
    ctx: &Context,
    enum_items: &str,
    load: impl Fn(&Bound<'py, PyAny>, &InstancePath, &Context) -> SerdeResult<Bound<'py, PyAny>>,
) -> SerdeResult<Bound<'py, PyAny>> {
    match parser.peek()? {
        Kind::Str => {
            let key = parser.take_pystring_known(py)?.into_any();
            load(&key, instance_path, ctx)
        }
        Kind::Num => {
            let key = match parser.take_number_known()? {
                ParsedNumber::Int(ParsedInt::I64(v)) => v.into_bound_py_any(py)?,
                ParsedNumber::Int(ParsedInt::Big(big)) => big.into_bound_py_any(py)?,
                ParsedNumber::F64(v) => PyFloat::new(py, v).into_any(),
            };
            load(&key, instance_path, ctx)
        }
        Kind::Bool => {
            let key = PyBool::new(py, parser.take_bool_known()?)
                .to_owned()
                .into_any();
            load(&key, instance_path, ctx)
        }
        _ => Err(wrong_enum_at_cursor(parser, enum_items, instance_path)),
    }
}

#[derive(Debug, Clone)]
pub struct DictionaryEncoder {
    pub(crate) key_encoder: Box<TEncoder>,
    pub(crate) value_encoder: Box<TEncoder>,
    pub(crate) omit_none: bool,
    /// Plain `str` key (no length bounds/custom encoder): the streaming load path
    /// uses the parsed key directly instead of re-validating via `key_encoder`.
    /// Measured: removing this costs ~3% Ir loading a 3-key `dict[str, int]` (500k iterations); see PR #272.
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
            // omit_none may drop entries, so the length is only known without it.
            writer.begin_map((!self.omit_none).then_some(dict.len()));
            for (k, v) in dict.iter() {
                if self.omit_none {
                    // Key is always dumped (validated) even when the None value is
                    // omitted; write key+value, then roll back if it encoded null.
                    let key = self.key_encoder.dump(&k, ctx)?;
                    let cp = writer.checkpoint();
                    write_map_key(&key, writer)?;
                    let value_start = writer.position();
                    self.value_encoder.dump_format(&v, writer, ctx)?;
                    if writer.tail_is_null(value_start) {
                        writer.rollback(cp);
                    } else {
                        writer.item_end();
                    }
                } else {
                    let key = self.key_encoder.dump(&k, ctx)?;
                    write_map_key(&key, writer)?;
                    self.value_encoder.dump_format(&v, writer, ctx)?;
                    writer.item_end();
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
            return Err(wrong_type_at_cursor(parser, "dict", instance_path));
        }
        // Materializing the key ends its borrow of the parser buffer and then
        // serves both instance_path and the insert.
        let (mut key_opt, len_hint) = parser.enter_map_known_sized()?;
        let result_dict = match len_hint {
            Some(len) => create_py_dict_known_size(py, len)?,
            None => PyDict::new(py),
        };
        while let Some(k) = key_opt {
            let py_key = create_py_string(py, k)?;
            let key_any = py_key.as_any();
            let item_path = instance_path.push(key_any);
            // `validated_key` keeps a non-plain key alive past the insert.
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

    #[inline]
    fn accepts_kind(&self, kind: Kind) -> bool {
        matches!(kind, Kind::Map)
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
            writer.begin_array(Some(list.len()));
            for index in 0..list.len() {
                let item = py_list_get_item(list, index)?;
                self.encoder.dump_format(&item, writer, ctx)?;
                writer.item_end();
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
            return Err(wrong_type_at_cursor(parser, "list", instance_path));
        }
        // Measured: `PyList::empty` + `append` instead of this Vec costs ~7% Ir on a 1000-element `list[int]` load; see PR #272.
        let mut items: Vec<Bound<'py, PyAny>> = Vec::new();
        if parser.enter_array_known()? {
            // One allocation instead of the regrowth ladder; empty arrays never
            // reach here and stay allocation-free. A format that states its length
            // (MessagePack) sizes this exactly — 8 is the guess for one that doesn't.
            items.reserve(parser.container_len_hint().unwrap_or(8));
            loop {
                // Length is only known at the closing bracket, so an element-type
                // error surfaces before a length error (dict path checks length up front).
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

    #[inline]
    fn accepts_kind(&self, kind: Kind) -> bool {
        matches!(kind, Kind::Array)
    }

    fn is_sequence(&self) -> bool {
        true
    }
}

/// Routing decision for one streamed object key, computed while the borrowed
/// `&str` is alive so the key is never copied to a `String`.
#[derive(Clone, Copy)]
enum Route {
    /// Key maps to `self.fields[idx]`.
    Field(usize),
    /// Unknown key — skip its value.
    Skip,
    /// End of object.
    End,
}

/// `expect` is the field following the one just filled: documents usually list
/// keys in schema order, so the common case is one string compare, not a hash
/// lookup; a miss falls through to the map.
#[inline]
fn resolve_route(
    fields: &[Field],
    routing: &FxHashMap<String, usize>,
    key: Option<&str>,
    expect: usize,
) -> Route {
    match key {
        Some(k) => {
            if let Some(field) = fields.get(expect) {
                if field.dict_key_rs == k {
                    return Route::Field(expect);
                }
            }
            match routing.get(k) {
                Some(&idx) => Route::Field(idx),
                None => Route::Skip,
            }
        }
        None => Route::End,
    }
}

/// Per-object "field seen" bitset, inline on the stack (<= 64 fields => one word).
struct SeenSet(SmallVec<[u64; 1]>);

impl SeenSet {
    #[inline(always)]
    fn new(field_count: usize) -> Self {
        SeenSet(smallvec![0u64; field_count.div_ceil(64)])
    }

    #[inline(always)]
    fn mark(&mut self, idx: usize) {
        self.0[idx >> 6] |= 1u64 << (idx & 63);
    }

    #[inline(always)]
    fn contains(&self, idx: usize) -> bool {
        self.0[idx >> 6] & (1u64 << (idx & 63)) != 0
    }
}

/// Shared by `EntityEncoder` (-> class instance) and `TypedDictEncoder` (->
/// dict): identical load/dump algorithm, only the "sink" differs. Sink hooks
/// are `#[inline(always)]` so each monomorphization is as if hand-written.
trait StreamingObject: Encoder {
    /// Schema type-mismatch label ("object" / "dict").
    const TYPE_NAME: &'static str;

    fn fields(&self) -> &[Field];
    fn format_routing(&self) -> &FxHashMap<String, usize>;
    fn used_keys(&self) -> &Py<PySet>;
    fn has_flatten(&self) -> bool;
    fn omit_none(&self) -> bool;
    /// `Some(len)` when every field always emits exactly one entry (no omit_none
    /// rollbacks, no skipped optional keys), letting sized formats write an
    /// exact map header. Precomputed at construction.
    fn dump_len_hint(&self) -> Option<usize>;

    // --- load sink: container typed per encoder, so `set` needs no re-cast;
    // `finish` erases it to `Bound<PyAny>` once. ---
    type Target<'py>
    where
        Self: 'py;
    fn create<'py>(&self, py: Python<'py>) -> SerdeResult<Self::Target<'py>>;
    fn set<'py>(
        &self,
        target: &Self::Target<'py>,
        field: &Field,
        val: Bound<'py, PyAny>,
    ) -> SerdeResult<()>;
    fn finish<'py>(target: Self::Target<'py>) -> Bound<'py, PyAny>;

    // --- dump source (type-erased; each impl narrows internally) ---
    /// Validate `value` before the field loop (TypedDict: must be a dict; Entity:
    /// no-op — missing attrs error per field).
    fn check_dump_source(&self, value: &Bound<'_, PyAny>) -> SerdeResult<()>;
    /// Fetch a field's value for dumping; `None` -> skip the field (no key).
    fn fetch<'py>(
        &self,
        value: &Bound<'py, PyAny>,
        field: &Field,
    ) -> SerdeResult<Option<Bound<'py, PyAny>>>;
}

/// Stream an object into `S`'s target, avoiding the dict-path's intermediate PyDict.
fn load_object_streaming<'py, S: StreamingObject + 'py>(
    enc: &S,
    py: Python<'py>,
    parser: &mut Parser<'_>,
    instance_path: &InstancePath,
    ctx: &Context,
) -> SerdeResult<Bound<'py, PyAny>> {
    let _guard = ctx.enter_depth()?;
    if parser.peek()? != Kind::Map {
        return Err(wrong_type_at_cursor(parser, S::TYPE_NAME, instance_path));
    }
    let target = enc.create(py)?;
    let fields = enc.fields();
    let mut seen = SeenSet::new(fields.len());
    if enc.has_flatten() {
        let unknowns = PyDict::new(py);
        let mut key = parser.enter_map_known()?;
        while let Some(k) = key {
            match enc.format_routing().get(k) {
                Some(&idx) => {
                    let field = &fields[idx];
                    let field_path = instance_path.push(field.dict_key.bind(py).as_any());
                    let val = field.encoder.load_format(py, parser, &field_path, ctx)?;
                    enc.set(&target, field, val)?;
                    seen.mark(idx);
                }
                None => {
                    // Unknown key -> a flatten field's: materialize only this value.
                    let py_key = create_py_string(py, k)?;
                    let v = parse_any(py, parser, ctx)?;
                    unknowns.set_item(py_key, v)?;
                }
            }
            key = parser.next_key()?;
        }
        for (idx, field) in fields.iter().enumerate() {
            let val = if field.is_flattened {
                field.load_value(&unknowns, instance_path, ctx, enc.used_keys())?
            } else if !seen.contains(idx) {
                field.get_default(py, instance_path)?
            } else {
                continue; // already set from the stream
            };
            enc.set(&target, field, val)?;
        }
    } else {
        let mut expect = 0;
        let mut route = resolve_route(
            fields,
            enc.format_routing(),
            parser.enter_map_known()?,
            expect,
        );
        loop {
            match route {
                Route::End => break,
                Route::Field(idx) => {
                    let field = &fields[idx];
                    let field_path = instance_path.push(field.dict_key.bind(py).as_any());
                    let val = field.encoder.load_format(py, parser, &field_path, ctx)?;
                    enc.set(&target, field, val)?;
                    seen.mark(idx);
                    expect = idx + 1;
                }
                Route::Skip => parser.skip_value()?,
            }
            route = resolve_route(fields, enc.format_routing(), parser.next_key()?, expect);
        }
        for (idx, field) in fields.iter().enumerate() {
            if !seen.contains(idx) {
                let val = field.get_default(py, instance_path)?;
                enc.set(&target, field, val)?;
            }
        }
    }
    Ok(S::finish(target))
}

/// Stream an object to the writer, avoiding the dict-path's intermediate PyDict.
/// Flatten objects keep parity via the bridge (materialize + `write_any`).
fn dump_object_streaming<S: StreamingObject>(
    enc: &S,
    value: &Bound<'_, PyAny>,
    writer: &mut Writer,
    ctx: &Context,
) -> SerdeResult<()> {
    if enc.has_flatten() {
        let dumped = enc.dump(value, ctx)?;
        return write_any(&dumped, writer, ctx);
    }
    let _guard = ctx.enter_depth()?;
    enc.check_dump_source(value)?;
    writer.begin_map(enc.dump_len_hint());
    for field in enc.fields() {
        let Some(field_val) = enc.fetch(value, field)? else {
            continue;
        };
        // Mirror the dict-path write condition: only optional fields under
        // omit_none need the dumped value first.
        if !field.required && enc.omit_none() {
            dump_field_unless_null(writer, &field.dump_key, &*field.encoder, &field_val, ctx)?;
        } else {
            writer.map_key_encoded(&field.dump_key);
            field.encoder.dump_format(&field_val, writer, ctx)?;
            writer.item_end();
        }
    }
    writer.end_map();
    Ok(())
}

/// The dict-path (non-codec) object load, reusing the same `StreamingObject` sinks.
fn load_dict_path<'a, S: StreamingObject + 'a>(
    enc: &S,
    value: &Bound<'a, PyAny>,
    instance_path: &InstancePath,
    ctx: &Context,
) -> SerdeResult<Bound<'a, PyAny>> {
    let _guard = ctx.enter_depth()?;
    let Ok(dict) = value.cast::<PyDict>() else {
        return Err(invalid_type_err(S::TYPE_NAME, value, instance_path));
    };
    let target = enc.create(value.py())?;
    for field in enc.fields() {
        let val = field.load_value(dict, instance_path, ctx, enc.used_keys())?;
        enc.set(&target, field, val)?;
    }
    Ok(S::finish(target))
}

#[derive(Debug, Clone)]
pub struct EntityEncoder {
    pub(crate) cls: Py<PyType>,
    pub(crate) omit_none: bool,
    pub(crate) is_frozen: bool,
    pub(crate) fields: Vec<Field>,
    pub(crate) used_keys: Py<PySet>,
    /// JSON key -> field index (non-flatten only); used by the streaming load path.
    pub(crate) format_routing: FxHashMap<String, usize>,
    /// Cached `any(is_flattened)` so the format hot paths don't rescan per call.
    pub(crate) has_flatten: bool,
    /// Every dump emits exactly `fields.len()` entries (no omit_none on optional
    /// fields), so sized formats can write an exact map header.
    pub(crate) dump_sized: bool,
}

#[derive(Debug, Clone)]
pub struct Field {
    pub(crate) name: Py<PyString>,
    pub(crate) dict_key: Py<PyString>,
    pub(crate) dict_key_rs: String,
    /// `dict_key_rs` pre-rendered for the streaming dump path (escaped once).
    pub(crate) dump_key: EncodedKey,
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
            (None, _) => Err(missing_required_property(
                self.dict_key.bind(py),
                instance_path,
            )),
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

impl EntityEncoder {
    /// Set a field on the instance; frozen entities disallow the fast unchecked setattr.
    #[inline(always)]
    fn set_field(
        &self,
        obj: &Bound<'_, PyAny>,
        field: &Field,
        val: Bound<'_, PyAny>,
    ) -> SerdeResult<()> {
        if self.is_frozen {
            generic_set_attr(obj, field.name.as_ptr(), val)?;
        } else {
            set_attr_unchecked(obj, field.name.as_ptr(), val)?;
        }
        Ok(())
    }
}

impl Encoder for EntityEncoder {
    #[inline]
    fn dump<'a>(&self, value: &Bound<'a, PyAny>, ctx: &Context) -> SerdeResult<Bound<'a, PyAny>> {
        let _guard = ctx.enter_depth()?;
        let dict = create_py_dict_known_size(value.py(), self.fields.len())?;
        for field in &self.fields {
            let field_val = match value.getattr(&field.name) {
                Ok(v) => v,
                // Missing attr means `value` isn't this entity's shape: surface a Schema
                // mismatch (not AttributeError) so an untagged union skips on, keeping
                // the original as `cause` since it may come from a user property.
                Err(e) if e.is_instance_of::<PyAttributeError>(value.py()) => {
                    // `cls.__name__` is read only if this error is rendered.
                    return Err(invalid_type_dump_err_with_cause(
                        self.cls.clone_ref(value.py()),
                        value,
                        e,
                    ));
                }
                Err(e) => return Err(e.into()),
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
        load_dict_path(self, value, instance_path, ctx)
    }

    fn dump_format(
        &self,
        value: &Bound<'_, PyAny>,
        writer: &mut Writer,
        ctx: &Context,
    ) -> SerdeResult<()> {
        dump_object_streaming(self, value, writer, ctx)
    }

    fn load_format<'py>(
        &self,
        py: Python<'py>,
        parser: &mut Parser<'_>,
        instance_path: &InstancePath,
        ctx: &Context,
    ) -> SerdeResult<Bound<'py, PyAny>> {
        load_object_streaming(self, py, parser, instance_path, ctx)
    }

    #[inline]
    fn accepts_kind(&self, kind: Kind) -> bool {
        matches!(kind, Kind::Map)
    }

    fn as_container_encoder(&self) -> Option<&dyn ContainerEncoder> {
        Some(self)
    }
}

impl StreamingObject for EntityEncoder {
    type Target<'py> = Bound<'py, PyAny>;
    const TYPE_NAME: &'static str = "object";

    #[inline(always)]
    fn fields(&self) -> &[Field] {
        &self.fields
    }
    #[inline(always)]
    fn format_routing(&self) -> &FxHashMap<String, usize> {
        &self.format_routing
    }
    #[inline(always)]
    fn used_keys(&self) -> &Py<PySet> {
        &self.used_keys
    }
    #[inline(always)]
    fn has_flatten(&self) -> bool {
        self.has_flatten
    }
    #[inline(always)]
    fn omit_none(&self) -> bool {
        self.omit_none
    }
    #[inline(always)]
    fn dump_len_hint(&self) -> Option<usize> {
        self.dump_sized.then_some(self.fields.len())
    }

    #[inline(always)]
    fn create<'py>(&self, py: Python<'py>) -> SerdeResult<Self::Target<'py>> {
        Ok(create_instance(self.cls.bind(py))?)
    }
    #[inline(always)]
    fn set<'py>(
        &self,
        target: &Self::Target<'py>,
        field: &Field,
        val: Bound<'py, PyAny>,
    ) -> SerdeResult<()> {
        self.set_field(target, field, val)
    }
    #[inline(always)]
    fn finish<'py>(target: Self::Target<'py>) -> Bound<'py, PyAny> {
        target
    }

    #[inline(always)]
    fn check_dump_source(&self, _value: &Bound<'_, PyAny>) -> SerdeResult<()> {
        // Any object works — a missing attribute surfaces per field in `fetch`.
        Ok(())
    }
    #[inline(always)]
    fn fetch<'py>(
        &self,
        value: &Bound<'py, PyAny>,
        field: &Field,
    ) -> SerdeResult<Option<Bound<'py, PyAny>>> {
        // Same AttributeError -> Schema-mismatch conversion as `dump`, for the
        // streaming dump path.
        match value.getattr(&field.name) {
            Ok(v) => Ok(Some(v)),
            Err(e) if e.is_instance_of::<PyAttributeError>(value.py()) => {
                // `cls.__name__` is read only if this error is rendered.
                Err(invalid_type_dump_err_with_cause(
                    self.cls.clone_ref(value.py()),
                    value,
                    e,
                ))
            }
            Err(e) => Err(e.into()),
        }
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
    /// JSON key -> field index (non-flatten only); used by the streaming load path.
    pub(crate) format_routing: FxHashMap<String, usize>,
    /// Cached `any(is_flattened)` so the format hot paths don't rescan per call.
    pub(crate) has_flatten: bool,
    /// Every dump emits exactly `fields.len()` entries (all fields required, so
    /// no skipped optional keys and no omit_none rollbacks).
    pub(crate) dump_sized: bool,
}

/// A required key absent from the dumped dict means `value` isn't this `TypedDict`'s
/// shape: a Schema mismatch, so an untagged union tries the next member instead of
/// aborting the dump. Shared by the dict path (`dump`) and the streaming one (`fetch`).
#[cold]
fn missing_dict_key_err(py: Python<'_>, field: &Field) -> SerdeError {
    SchemaError::deferred(
        Message::MissingDictKey {
            name: field.name.clone_ref(py),
        },
        &InstancePath::new(),
    )
    .into()
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
                        return Err(missing_dict_key_err(value.py(), field));
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
        load_dict_path(self, value, instance_path, ctx)
    }

    fn dump_format(
        &self,
        value: &Bound<'_, PyAny>,
        writer: &mut Writer,
        ctx: &Context,
    ) -> SerdeResult<()> {
        dump_object_streaming(self, value, writer, ctx)
    }

    fn load_format<'py>(
        &self,
        py: Python<'py>,
        parser: &mut Parser<'_>,
        instance_path: &InstancePath,
        ctx: &Context,
    ) -> SerdeResult<Bound<'py, PyAny>> {
        load_object_streaming(self, py, parser, instance_path, ctx)
    }

    #[inline]
    fn accepts_kind(&self, kind: Kind) -> bool {
        matches!(kind, Kind::Map)
    }

    fn as_container_encoder(&self) -> Option<&dyn ContainerEncoder> {
        Some(self)
    }
}

impl StreamingObject for TypedDictEncoder {
    type Target<'py> = Bound<'py, PyDict>;
    const TYPE_NAME: &'static str = "dict";

    #[inline(always)]
    fn fields(&self) -> &[Field] {
        &self.fields
    }
    #[inline(always)]
    fn format_routing(&self) -> &FxHashMap<String, usize> {
        &self.format_routing
    }
    #[inline(always)]
    fn used_keys(&self) -> &Py<PySet> {
        &self.used_keys
    }
    #[inline(always)]
    fn has_flatten(&self) -> bool {
        self.has_flatten
    }
    #[inline(always)]
    fn omit_none(&self) -> bool {
        self.omit_none
    }
    #[inline(always)]
    fn dump_len_hint(&self) -> Option<usize> {
        self.dump_sized.then_some(self.fields.len())
    }

    #[inline(always)]
    fn create<'py>(&self, py: Python<'py>) -> SerdeResult<Self::Target<'py>> {
        Ok(create_py_dict_known_size(py, self.fields.len())?)
    }
    #[inline(always)]
    fn set<'py>(
        &self,
        target: &Self::Target<'py>,
        field: &Field,
        val: Bound<'py, PyAny>,
    ) -> SerdeResult<()> {
        // `target` is already the concrete `PyDict` (typed GAT), so no re-cast.
        py_dict_set_item(target, field.name.as_ptr(), val)?;
        Ok(())
    }
    #[inline(always)]
    fn finish<'py>(target: Self::Target<'py>) -> Bound<'py, PyAny> {
        target.into_any()
    }

    #[inline(always)]
    fn check_dump_source(&self, value: &Bound<'_, PyAny>) -> SerdeResult<()> {
        match value.cast::<PyDict>() {
            Ok(_) => Ok(()),
            _ => Err(invalid_type_dump_err("dict", value)),
        }
    }
    #[inline(always)]
    fn fetch<'py>(
        &self,
        value: &Bound<'py, PyAny>,
        field: &Field,
    ) -> SerdeResult<Option<Bound<'py, PyAny>>> {
        // `check_dump_source` already verified `value` is a dict; recover it.
        let dict = value.cast::<PyDict>().map_err(PyErr::from)?;
        match dict.get_item(&field.name) {
            Ok(Some(val)) => Ok(Some(val)),
            _ => {
                if field.required {
                    Err(missing_dict_key_err(dict.py(), field))
                } else {
                    // Missing optional key: skip entirely (no key emitted).
                    Ok(None)
                }
            }
        }
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
            return Err(wrong_type_err("uuid", s, instance_path));
        }
        Err(wrong_type_at_cursor(parser, "uuid", instance_path))
    }

    #[inline]
    fn accepts_kind(&self, kind: Kind) -> bool {
        matches!(kind, Kind::Str)
    }
}

#[derive(Debug, Clone)]
pub struct EnumEncoder {
    pub(crate) enum_items: Arc<str>,
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

    // Same lookup as `dump`, but streams the resolved scalar directly.
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

    fn load_format<'py>(
        &self,
        py: Python<'py>,
        parser: &mut Parser<'_>,
        instance_path: &InstancePath,
        ctx: &Context,
    ) -> SerdeResult<Bound<'py, PyAny>> {
        load_enum_scalar(
            py,
            parser,
            instance_path,
            ctx,
            &self.enum_items,
            |v, p, c| self.load(v, p, c),
        )
    }

    #[inline]
    fn accepts_kind(&self, kind: Kind) -> bool {
        matches!(kind, Kind::Str | Kind::Num | Kind::Bool)
    }
}

#[derive(Debug, Clone)]
pub struct LiteralEncoder {
    pub(crate) enum_items: Arc<str>,
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

    // Same lookup as `dump`, but streams the resolved scalar directly.
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

    fn load_format<'py>(
        &self,
        py: Python<'py>,
        parser: &mut Parser<'_>,
        instance_path: &InstancePath,
        ctx: &Context,
    ) -> SerdeResult<Bound<'py, PyAny>> {
        load_enum_scalar(
            py,
            parser,
            instance_path,
            ctx,
            &self.enum_items,
            |v, p, c| self.load(v, p, c),
        )
    }

    #[inline]
    fn accepts_kind(&self, kind: Kind) -> bool {
        matches!(kind, Kind::Str | Kind::Num | Kind::Bool)
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

    #[inline]
    fn accepts_kind(&self, kind: Kind) -> bool {
        matches!(kind, Kind::Null) || self.encoder.accepts_kind(kind)
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
            writer.begin_array(Some(seq_len));
            for index in 0..seq_len {
                let item = seq.get_item(index)?;
                self.encoders[index].dump_format(&item, writer, ctx)?;
                writer.item_end();
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
            return Err(wrong_type_at_cursor(parser, "sequence", instance_path));
        }
        // The schema fixes the arity, so size the buffer exactly once.
        let mut items: Vec<Bound<'py, PyAny>> = Vec::with_capacity(self.encoders.len());
        if parser.enter_array_known()? {
            loop {
                let idx = items.len();
                if idx < self.encoders.len() {
                    let item_path = instance_path.push(idx);
                    items.push(self.encoders[idx].load_format(py, parser, &item_path, ctx)?);
                } else {
                    // Extra items: consume generically so the final count triggers
                    // the same "has more than N items" error as check_sequence_size.
                    items.push(parse_any(py, parser, ctx)?);
                }
                if !parser.next_array_item()? {
                    break;
                }
            }
        }
        // Length is only known at the closing bracket, so an item type error
        // surfaces before a length error (dict path checks length up front).
        if sequence_size_ordering(items.len(), self.encoders.len()) != Ordering::Equal {
            // Arity mismatch is the cold path: a `PyList` is built only now,
            // solely so the error message can reuse the sequence's `str()`
            // instead of duplicating that formatting here.
            let list = PyList::new(py, items)?;
            let seq = list.cast::<PySequence>().map_err(PyErr::from)?;
            return Err(sequence_size_err(
                seq,
                list.len(),
                self.encoders.len(),
                Some(instance_path),
            ));
        }
        // Common case: sizes match, so build the tuple directly from the
        // Vec, moving each already-owned reference straight in — no
        // intermediate PyList/PySequence allocation.
        let result = create_py_tuple(py, items.len())?;
        for (index, item) in items.into_iter().enumerate() {
            py_tuple_set_item(&result, index, item);
        }
        Ok(result.into_any())
    }

    #[inline]
    fn accepts_kind(&self, kind: Kind) -> bool {
        matches!(kind, Kind::Array)
    }

    fn is_sequence(&self) -> bool {
        true
    }
}

#[derive(Debug, Clone)]
pub struct UnionEncoder {
    pub(crate) encoders: Vec<Box<TEncoder>>,
    pub(crate) repr: Arc<str>,
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
        Err(invalid_type_dump_err(Arc::clone(&self.repr), value))
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
        Err(invalid_type_err(
            Arc::clone(&self.repr),
            value,
            instance_path,
        ))
    }

    // Probe each member's *validating* dump_format; the first that succeeds is
    // kept. The bridge default is unusable: self.dump's scalar members don't
    // type-validate (they clone), so `int | Entity` would wrongly take int.
    fn dump_format(
        &self,
        value: &Bound<'_, PyAny>,
        writer: &mut Writer,
        ctx: &Context,
    ) -> SerdeResult<()> {
        for encoder in &self.encoders {
            let cp = writer.checkpoint();
            match encoder.dump_format(value, writer, ctx) {
                Ok(()) => return Ok(()),
                Err(SerdeError::Schema(_)) => {
                    writer.rollback(cp);
                    continue;
                }
                Err(e @ SerdeError::Py(_)) => return Err(e),
            }
        }
        Err(invalid_type_dump_err(Arc::clone(&self.repr), value))
    }

    fn load_format<'py>(
        &self,
        py: Python<'py>,
        parser: &mut Parser<'_>,
        instance_path: &InstancePath,
        ctx: &Context,
    ) -> SerdeResult<Bound<'py, PyAny>> {
        // A kind that narrows the union to one member is read straight off the
        // cursor — no span capture, no re-parse. Untagged unions usually mix kinds.
        let kind = parser.peek()?;
        let mut only: Option<&Box<TEncoder>> = None;
        let mut viable = 0usize;
        for encoder in &self.encoders {
            if encoder.accepts_kind(kind) {
                viable += 1;
                if viable > 1 {
                    break;
                }
                only = Some(encoder);
            }
        }
        if viable == 1 {
            let encoder = only.expect("viable == 1");
            // Nothing else could have matched this kind, so the member's own error
            // surfaces — more specific than the union's, and a deliberate divergence
            // from the dict path, which reports "nothing matched" at the root.
            return encoder.load_format(py, parser, instance_path, ctx);
        }

        // `take_raw_value` already advanced the main cursor, so a member that
        // consumes its sub-parser only partially cannot corrupt it.
        let span = parser.take_raw_value()?;
        for encoder in &self.encoders {
            if !encoder.accepts_kind(kind) {
                continue;
            }
            let mut sub = parser.sub_parser(span);
            match encoder.load_format(py, &mut sub, instance_path, ctx) {
                Ok(v) => return Ok(v),
                Err(SerdeError::Schema(_)) => continue,
                Err(e @ SerdeError::Py(_)) => return Err(e),
            }
        }
        let raw = parser.value_repr(span)?;
        Err(wrong_type_err(&self.repr, &raw, instance_path))
    }

    #[inline]
    fn accepts_kind(&self, kind: Kind) -> bool {
        self.encoders.iter().any(|e| e.accepts_kind(kind))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct DiscriminatorKey(String);

// Lets `HashMap<DiscriminatorKey, _>` be probed by a borrowed `&str` (no
// throwaway key alloc). Hash/Eq are `str`-consistent via the inner `String`.
impl std::borrow::Borrow<str> for DiscriminatorKey {
    fn borrow(&self) -> &str {
        &self.0
    }
}

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
                    self.dump_discriminator.bind(value.py()),
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
                        self.load_discriminator.bind(val.py()),
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

    // dump_format stays on the bridge default (self.dump selects the variant).

    fn load_format<'py>(
        &self,
        py: Python<'py>,
        parser: &mut Parser<'_>,
        instance_path: &InstancePath,
        ctx: &Context,
    ) -> SerdeResult<Bound<'py, PyAny>> {
        if parser.peek()? != Kind::Map {
            return Err(wrong_type_at_cursor(parser, "dict", instance_path));
        }
        let span = parser.take_raw_value()?;
        // Scan a throwaway sub-parser for the discriminator, regardless of key order.
        let mut tag: Option<String> = None;
        {
            let mut scan = parser.sub_parser(span);
            // Keys stay borrowed: the borrow ends at the comparison, before the
            // next `&mut scan`.
            let mut key = scan.enter_map()?;
            while let Some(k) = key {
                if k == self.load_discriminator_rs {
                    if scan.peek()? == Kind::Str {
                        // `to_owned`: the tag outlives this scan sub-parser.
                        tag = Some(scan.take_str_known()?.to_owned());
                    }
                    break;
                }
                scan.skip_value()?;
                key = scan.next_key()?;
            }
        }
        let Some(tag) = tag else {
            // Missing/non-string discriminator: re-run the object path for the exact error.
            let mut sub = parser.sub_parser(span);
            let value = parse_any(py, &mut sub, ctx)?;
            return self.load(&value, instance_path, ctx);
        };
        // Select by tag; unknown tag -> same Schema error as the object path.
        match self.encoders.get(tag.as_str()) {
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

    #[inline]
    fn accepts_kind(&self, kind: Kind) -> bool {
        matches!(kind, Kind::Map)
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
            return Err(wrong_type_err("time", s, instance_path));
        }
        Err(wrong_type_at_cursor(parser, "time", instance_path))
    }

    #[inline]
    fn accepts_kind(&self, kind: Kind) -> bool {
        matches!(kind, Kind::Str)
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
            return Err(wrong_type_err("datetime", s, instance_path));
        }
        Err(wrong_type_at_cursor(parser, "datetime", instance_path))
    }

    #[inline]
    fn accepts_kind(&self, kind: Kind) -> bool {
        matches!(kind, Kind::Str)
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
            return Err(wrong_type_err("date", s, instance_path));
        }
        Err(wrong_type_at_cursor(parser, "date", instance_path))
    }

    #[inline]
    fn accepts_kind(&self, kind: Kind) -> bool {
        matches!(kind, Kind::Str)
    }
}

/// Placeholder for a recursive encoder: for a self-referential type we hand out
/// a `LazyEncoder` and back-fill the inner `Arc` once the outer encoder is built.
/// `OnceLock` makes the back-fill thread-safe under free-threaded Python (written
/// once during `Serializer::new`, read concurrently after).
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

    // With a user callback, materialize (run it, then bridge); without one,
    // delegate to inner's format methods to keep its direct optimization.
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
