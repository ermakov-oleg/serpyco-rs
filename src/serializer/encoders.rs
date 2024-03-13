use std::collections::HashMap;
use std::fmt::Debug;
use std::sync::Arc;

use atomic_refcell::AtomicRefCell;
use dyn_clone::{clone_trait_object, DynClone};
use pyo3::{Bound, intern, Py, PyAny, PyResult};
use pyo3::exceptions::PyRuntimeError;
use pyo3::prelude::*;
use pyo3::types::{PyDateTime, PyDict, PyList, PySequence, PyString};
use pyo3_ffi::PyObject;
use uuid::Uuid;

use crate::errors::{ToPyErr, ValidationError};
use crate::python::{create_new_object, create_py_list, get_none, parse_date, parse_datetime, parse_time, py_dict_set_item, py_frozen_object_set_attr, py_list_get_item, py_list_set_item, py_object_call1_make_tuple_or_err, py_object_get_item, py_object_set_attr, py_str_to_str, to_decimal, to_uuid};
use crate::python::macros::{call_object, ffi};
use crate::validator::{
    Array as PyArray, Context, Dict as PyDictOld, InstancePath, Sequence, Tuple as PyTuple,
    Value as PyValue,
};
use crate::validator::types::{DecimalType, EnumItem, FloatType, IntegerType, StringType};
use crate::validator::validators::{check_bounds, check_length, check_sequence_size, check_sequence_size_, invalid_enum_item, invalid_type, invalid_type_dump, missing_required_property, no_encoder_for_discriminator};

pub type TEncoder = dyn Encoder + Send + Sync;

pub trait Encoder: DynClone + Debug {
    fn dump<'a>(&self, value: &Bound<'a, PyAny>) -> PyResult<Bound<'a, PyAny>>;
    fn load(&self, value: *mut PyObject, instance_path: &InstancePath) -> PyResult<*mut PyObject>;
}

clone_trait_object!(Encoder);

#[derive(Debug, Clone)]
pub struct NoopEncoder;

impl Encoder for NoopEncoder {
    #[inline]
    fn dump<'a>(&self, value: &Bound<'a, PyAny>) -> PyResult<Bound<'a, PyAny>> {
        Ok(value.clone())
    }

    #[inline]
    fn load(&self, value: *mut PyObject, _instance_path: &InstancePath) -> PyResult<*mut PyObject> {
        ffi!(Py_INCREF(value));
        Ok(value)
    }
}

#[derive(Debug, Clone)]
pub struct IntEncoder {
    pub(crate) type_info: IntegerType,
    #[allow(dead_code)]
    pub(crate) ctx: Context,
}

impl Encoder for IntEncoder {
    #[inline]
    fn dump<'a>(&self, value: &Bound<'a, PyAny>) -> PyResult<Bound<'a, PyAny>> {
        Ok(value.clone())
    }

    #[inline]
    fn load(&self, value: *mut PyObject, instance_path: &InstancePath) -> PyResult<*mut PyObject> {
        let val = PyValue::new(value);

        if val.is_int() {
            check_bounds(val, self.type_info.min, self.type_info.max, instance_path)?;
            ffi!(Py_INCREF(value));
            Ok(value)
        } else {
            invalid_type!("integer", val, instance_path)
        }
    }
}

#[derive(Debug, Clone)]
pub struct FloatEncoder {
    pub(crate) type_info: FloatType,
    #[allow(dead_code)]
    pub(crate) ctx: Context,
}

impl Encoder for FloatEncoder {
    #[inline]
    fn dump<'a>(&self, value: &Bound<'a, PyAny>) -> PyResult<Bound<'a, PyAny>> {
        Ok(value.clone())
    }

    #[inline]
    fn load(&self, value: *mut PyObject, instance_path: &InstancePath) -> PyResult<*mut PyObject> {
        let py_val = PyValue::new(value);
        if py_val.is_number() {
            check_bounds(
                py_val,
                self.type_info.min,
                self.type_info.max,
                instance_path,
            )?;
            ffi!(Py_INCREF(value));
            Ok(value)
        } else {
            invalid_type!("number", py_val, instance_path)
        }
    }
}

#[derive(Debug, Clone)]
pub struct DecimalEncoder {
    pub(crate) type_info: DecimalType,
    #[allow(dead_code)]
    pub(crate) ctx: Context,
}

impl Encoder for DecimalEncoder {
    #[inline]
    fn dump<'a>(&self, value: &Bound<'a, PyAny>) -> PyResult<Bound<'a, PyAny>> {
        Ok(value.str()?.into_any())
    }

    #[inline]
    fn load(&self, value: *mut PyObject, instance_path: &InstancePath) -> PyResult<*mut PyObject> {
        let py_val = PyValue::new(value);

        if let Some(val) = py_val.maybe_number() {
            check_bounds(val, self.type_info.min, self.type_info.max, instance_path)?;
            let result = to_decimal(value).expect("decimal");
            Ok(result)
        } else {
            invalid_type!("decimal", py_val, instance_path)
        }
    }
}

#[derive(Debug, Clone)]
pub struct StringEncoder {
    pub(crate) type_info: StringType,
    #[allow(dead_code)]
    pub(crate) ctx: Context,
}

impl Encoder for StringEncoder {

    #[inline]
    fn dump<'a>(&self, value: &Bound<'a, PyAny>) -> PyResult<Bound<'a, PyAny>> {
        Ok(value.clone())
    }

    #[inline]
    fn load(&self, value: *mut PyObject, instance_path: &InstancePath) -> PyResult<*mut PyObject> {
        let val = PyValue::new(value);
        if val.is_string() {
            check_length(
                &val,
                self.type_info.min_length,
                self.type_info.max_length,
                instance_path,
            )?;
            ffi!(Py_INCREF(value));
            Ok(value)
        } else {
            invalid_type!("string", val, instance_path)
        }
    }
}

#[derive(Debug, Clone)]
pub struct BooleanEncoder {
    #[allow(dead_code)]
    pub(crate) ctx: Context,
}

impl Encoder for BooleanEncoder {
    #[inline]
    fn dump<'a>(&self, value: &Bound<'a, PyAny>) -> PyResult<Bound<'a, PyAny>> {
        Ok(value.clone())
    }

    #[inline]
    fn load(&self, value: *mut PyObject, instance_path: &InstancePath) -> PyResult<*mut PyObject> {
        let py_val = PyValue::new(value);
        if py_val.as_bool().is_some() {
            ffi!(Py_INCREF(value));
            Ok(value)
        } else {
            invalid_type!("boolean", py_val, instance_path)
        }
    }
}

#[derive(Debug, Clone)]
pub struct BytesEncoder {
    #[allow(dead_code)]
    pub(crate) ctx: Context,
}

impl Encoder for BytesEncoder {
    #[inline]
    fn dump<'a>(&self, value: &Bound<'a, PyAny>) -> PyResult<Bound<'a, PyAny>> {
        Ok(value.clone())
    }

    #[inline]
    fn load(&self, value: *mut PyObject, instance_path: &InstancePath) -> PyResult<*mut PyObject> {
        let py_value = PyValue::new(value);
        if py_value.is_bytes() {
            ffi!(Py_INCREF(value));
            Ok(value)
        } else {
            invalid_type!("bytes", py_value, instance_path)
        }
    }
}

#[derive(Debug, Clone)]
pub struct DictionaryEncoder {
    pub(crate) key_encoder: Box<TEncoder>,
    pub(crate) value_encoder: Box<TEncoder>,
    pub(crate) omit_none: bool,
    #[allow(dead_code)]
    pub(crate) ctx: Context,
}

impl Encoder for DictionaryEncoder {
    #[inline]
    fn dump<'a>(&self, value: &Bound<'a, PyAny>) -> PyResult<Bound<'a, PyAny>> {
        if let Ok(dict) = value.downcast::<PyDict>() {
            let result_dict = PyDict::new_bound(dict.py());
            for (k, v) in dict.iter() {
                let key = self.key_encoder.dump(&k)?;
                let value = self.value_encoder.dump(&v)?;
                if !self.omit_none || !value.is_none() {
                    // py_dict_set_item(&result_dict, key.as_ptr(), value)?;
                    result_dict.set_item(key, value)?;
                }
            }
            Ok(result_dict.into_any())
        } else {
            invalid_type_dump!("dict", value)
        }
    }

    #[inline]
    fn load(&self, value: *mut PyObject, instance_path: &InstancePath) -> PyResult<*mut PyObject> {
        let py_value = PyValue::new(value);
        if let Some(dict) = py_value.as_dict() {
            let mut result_dict = PyDictOld::new_empty();
            for i in dict.iter()? {
                let (k, v) = i?;
                let instance_path = instance_path.push(&k);
                let key = self.key_encoder.load(k.as_ptr(), &instance_path)?;
                let value = self.value_encoder.load(v.as_ptr(), &instance_path)?;
                result_dict.set(key, value)?;
            }
            Ok(result_dict.as_ptr())
        } else {
            invalid_type!("object", py_value, instance_path)
        }
    }
}

#[derive(Debug, Clone)]
pub struct ArrayEncoder {
    pub(crate) encoder: Box<TEncoder>,
    #[allow(dead_code)]
    pub(crate) ctx: Context,
}

impl Encoder for ArrayEncoder {
    #[inline]
    fn dump<'a>(&self, value: &Bound<'a, PyAny>) -> PyResult<Bound<'a, PyAny>> {
        if let Ok(list) = value.downcast::<PyList>() {
            let size = list.len();
            let result = create_py_list(value.py(), size);

            for index in 0..size {
                let item = py_list_get_item(list, index);
                let val = self.encoder.dump(&item)?;
                py_list_set_item(&result, index, val);
            }

            Ok(result.into_any())

        } else {
            invalid_type_dump!("list", value)
        }
    }

    #[inline]
    fn load(&self, value: *mut PyObject, instance_path: &InstancePath) -> PyResult<*mut PyObject> {
        let py_value = PyValue::new(value);
        if let Some(val) = py_value.as_array() {
            let array: PyArray = val.map_into(&|i, item| {
                let instance_path = instance_path.push(i);
                self.encoder.load(item, &instance_path)
            })?;
            Ok(array.as_ptr())
        } else {
            invalid_type!("array", py_value, instance_path)
        }
    }
}

#[derive(Debug, Clone)]
pub struct EntityEncoder {
    pub(crate) cls: Py<PyAny>,
    pub(crate) omit_none: bool,
    pub(crate) is_frozen: bool,
    pub(crate) fields: Vec<Field>,
    #[allow(dead_code)]
    pub(crate) ctx: Context,
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
}

impl Encoder for EntityEncoder {
    #[inline]
    fn dump<'a>(&self, value: &Bound<'a, PyAny>) -> PyResult<Bound<'a, PyAny>> {
        let dict = PyDict::new_bound(value.py());

        for field in &self.fields {
            let field_val = value.getattr(&field.name)?;
            let dump_result = field.encoder.dump(&field_val)?;
            if field.required || !self.omit_none || !dump_result.is_none() {
                py_dict_set_item(&dict, field.dict_key.as_ptr(), dump_result)?;
            }
        }

        Ok(dict.into_any())
    }

    #[inline]
    fn load(&self, value: *mut PyObject, instance_path: &InstancePath) -> PyResult<*mut PyObject> {
        let py_value = PyValue::new(value);
        let setattr_fn = if self.is_frozen {
            py_frozen_object_set_attr
        } else {
            py_object_set_attr
        };
        if let Some(dict) = py_value.as_dict() {
            let obj = create_new_object(self.cls.as_ptr())?;

            for field in &self.fields {
                let val = match dict.get_item(field.dict_key.as_ptr()) {
                    Some(val) => {
                        let cur_path = PyValue::new(field.dict_key.as_ptr());
                        let instance_path = instance_path.push(&cur_path);
                        field.encoder.load(val.as_ptr(), &instance_path)?
                    }
                    None => match (&field.default, &field.default_factory) {
                        (Some(val), _) => {
                            let val = val.as_ptr();
                            ffi!(Py_INCREF(val));
                            val
                        }
                        (_, Some(val)) => call_object!(val.as_ptr())?,
                        (None, _) => {
                            return Err(missing_required_property(
                                &field.dict_key_rs,
                                instance_path,
                            ));
                        }
                    },
                };
                setattr_fn(obj, field.name.as_ptr(), val)?;
                // val RC +1
                ffi!(Py_DECREF(val));
            }
            Ok(obj)
        } else {
            invalid_type!("object", py_value, instance_path)
        }
    }
}

#[derive(Debug, Clone)]
pub struct TypedDictEncoder {
    pub(crate) omit_none: bool,
    pub(crate) fields: Vec<Field>,
    #[allow(dead_code)]
    pub(crate) ctx: Context,
}

impl Encoder for TypedDictEncoder {
    #[inline]
    fn dump<'a>(&self, value: &Bound<'a, PyAny>) -> PyResult<Bound<'a, PyAny>> {
        let dict = PyDict::new_bound(value.py());
        let value = match value.downcast::<PyDict>() {
            Ok(val) => val,
            Err(_) => {
                return invalid_type_dump!("dict", value);
            }
        };
        for field in &self.fields {
            let field_val = match value.get_item(&field.name) {
                Ok(Some(val)) => val,
                _ => {
                    if field.required {
                        return Err(ValidationError::new_err(format!(
                            "data dictionary is missing required parameter {}",
                            &field.name
                        )));
                    } else {
                        continue;
                    }
                }
            };
            let dump_result = field.encoder.dump(&field_val)?;
            if field.required || !self.omit_none || !dump_result.is_none() {
                py_dict_set_item(&dict, field.dict_key.as_ptr(), dump_result)?;
            }
        }
        Ok(dict.into_any())
    }

    #[inline]
    fn load(&self, value: *mut PyObject, instance_path: &InstancePath) -> PyResult<*mut PyObject> {
        let py_value = PyValue::new(value);
        if let Some(dict) = py_value.as_dict() {
            let dict_ptr = ffi!(PyDict_New());
            for field in &self.fields {
                let val = match dict.get_item(field.dict_key.as_ptr()) {
                    Some(val) => {
                        let cur_path = PyValue::new(field.dict_key.as_ptr());
                        let instance_path = instance_path.push(&cur_path);
                        field.encoder.load(val.as_ptr(), &instance_path)? // new obj or RC +1
                    }
                    None => {
                        if field.required {
                            return Err(missing_required_property(
                                &field.dict_key_rs,
                                instance_path,
                            ));
                        } else {
                            continue;
                        }
                    }
                };
                ffi!(PyDict_SetItem(dict_ptr, field.name.as_ptr(), val));
                // key and val RC +1
                ffi!(Py_DECREF(val));
            }
            Ok(dict_ptr)
        } else {
            invalid_type!("object", py_value, instance_path)
        }
    }
}

#[derive(Debug, Clone)]
pub struct UUIDEncoder {
    #[allow(dead_code)]
    pub(crate) ctx: Context,
}

impl Encoder for UUIDEncoder {
    #[inline]
    fn dump<'a>(&self, value: &Bound<'a, PyAny>) -> PyResult<Bound<'a, PyAny>> {
        Ok(value.str()?.into_any())
    }

    #[inline]
    fn load(&self, value: *mut PyObject, instance_path: &InstancePath) -> PyResult<*mut PyObject> {
        let py_val = PyValue::new(value);
        if let Some(val) = py_val.as_str() {
            if Uuid::parse_str(val).is_ok() {
                return to_uuid(value);
            }
        }
        invalid_type!("uuid", py_val, instance_path)
    }
}

#[derive(Debug, Clone)]
pub struct EnumEncoder {
    pub(crate) enum_items: Vec<(EnumItem, Py<PyAny>)>,
    #[allow(dead_code)]
    pub(crate) ctx: Context,
}

impl Encoder for EnumEncoder {
    #[inline]
    fn dump<'a>(&self, value: &Bound<'a, PyAny>) -> PyResult<Bound<'a, PyAny>> {
        let value_str = intern!(value.py(), "value");
        Ok(value.getattr(value_str)?.into_any())
    }

    #[inline]
    fn load(&self, value: *mut PyObject, instance_path: &InstancePath) -> PyResult<*mut PyObject> {
        let py_val = PyValue::new(value);
        let item = if let Some(val) = py_val.as_str() {
            EnumItem::String(val.to_string())
        } else if let Some(val) = py_val.as_int() {
            EnumItem::Int(val)
        } else {
            invalid_enum_item!((&self.enum_items).into(), py_val, instance_path);
        };
        let index = self.enum_items.binary_search_by(|(e, _)| e.cmp(&item));

        match index {
            Ok(index) => {
                let (_, py_item) = &self.enum_items[index];
                ffi!(Py_INCREF(py_item.as_ptr()));
                Ok(py_item.as_ptr())
            }
            Err(_) => {
                invalid_enum_item!((&self.enum_items).into(), py_val, instance_path);
            }
        }
    }
}

#[derive(Debug, Clone)]
pub struct LiteralEncoder {
    pub(crate) enum_items: Vec<(EnumItem, Py<PyAny>)>,
    #[allow(dead_code)]
    pub(crate) ctx: Context,
}

impl Encoder for LiteralEncoder {
    #[inline]
    fn dump<'a>(&self, value: &Bound<'a, PyAny>) -> PyResult<Bound<'a, PyAny>> {
        Ok(value.clone())
    }

    #[inline]
    fn load(&self, value: *mut PyObject, instance_path: &InstancePath) -> PyResult<*mut PyObject> {
        let py_val = PyValue::new(value);
        let item = if let Some(val) = py_val.as_str() {
            EnumItem::String(val.to_string())
        } else if let Some(val) = py_val.as_int() {
            EnumItem::Int(val)
        } else {
            invalid_enum_item!((&self.enum_items).into(), py_val, instance_path);
        };

        let index = self.enum_items.binary_search_by(|(e, _)| e.cmp(&item));
        match index {
            Ok(index) => {
                let (_, py_item) = &self.enum_items[index];
                ffi!(Py_INCREF(py_item.as_ptr()));
                Ok(py_item.as_ptr())
            }
            Err(_) => {
                invalid_enum_item!((&self.enum_items).into(), py_val, instance_path);
            }
        }
    }
}

#[derive(Debug, Clone)]
pub struct OptionalEncoder {
    pub(crate) encoder: Box<TEncoder>,
    #[allow(dead_code)]
    pub(crate) ctx: Context,
}

impl Encoder for OptionalEncoder {
    #[inline]
    fn dump<'a>(&self, value: &Bound<'a, PyAny>) -> PyResult<Bound<'a, PyAny>> {
        if value.is_none() {
            Ok(value.clone())
        } else {
            self.encoder.dump(value)
        }
    }

    #[inline]
    fn load(&self, value: *mut PyObject, instance_path: &InstancePath) -> PyResult<*mut PyObject> {
        let py_value = PyValue::new(value);
        if py_value.is_none() {
            Ok(get_none())
        } else {
            self.encoder.load(value, instance_path)
        }
    }
}

#[derive(Debug, Clone)]
pub struct TupleEncoder {
    pub(crate) encoders: Vec<Box<TEncoder>>,
    #[allow(dead_code)]
    pub(crate) ctx: Context,
}

impl Encoder for TupleEncoder {
    #[inline]
    fn dump<'a>(&self, value: &Bound<'a, PyAny>) -> PyResult<Bound<'a, PyAny>> {
        if let Ok(seq) = value.downcast::<PySequence>() {
            let seq_len = seq.len()?;
            check_sequence_size(&seq, seq_len, self.encoders.len(), None)?;
            let result = create_py_list(value.py(), seq_len);
            for index in 0..seq_len {
                let item = seq.get_item(index)?;
                let val = self.encoders[index].dump(&item)?;
                py_list_set_item(&result, index, val);
            }

            Ok(result.into_any())
        } else {
            invalid_type_dump!("sequence", value)
        }
    }

    #[inline]
    fn load(&self, value: *mut PyObject, instance_path: &InstancePath) -> PyResult<*mut PyObject> {
        let py_value = PyValue::new(value);
        match py_value.as_sequence() {
            None => {
                invalid_type!("sequence", py_value, instance_path)
            }
            Some(val) => {
                check_sequence_size_(&val, self.encoders.len() as isize, Some(instance_path))?;
                let tuple: PyTuple = val.map_into(&|i, item| {
                    let instance_path = instance_path.push(i);
                    self.encoders[i as usize].load(item, &instance_path)
                })?;
                Ok(tuple.as_ptr())
            }
        }
    }
}

#[derive(Debug, Clone)]
pub struct UnionEncoder {
    pub(crate) encoders: Vec<Box<TEncoder>>,
    pub(crate) union_repr: String,
    #[allow(dead_code)]
    pub(crate) ctx: Context,
}

impl Encoder for UnionEncoder {
    #[inline]
    fn dump<'a>(&self, value: &Bound<'a, PyAny>) -> PyResult<Bound<'a, PyAny>> {
        for encoder in &self.encoders {
            let result = encoder.dump(value);
            if result.is_ok() {
                return result;
            }
        }
        invalid_type_dump!(&self.union_repr, value)
    }

    #[inline]
    fn load(&self, value: *mut PyObject, instance_path: &InstancePath) -> PyResult<*mut PyObject> {
        for encoder in &self.encoders {
            let result = encoder.load(value, instance_path);
            if result.is_ok() {
                return result;
            }
        }
        invalid_type!(&self.union_repr, PyValue::new(value), instance_path)
    }
}

#[derive(Debug, Clone)]
pub struct DiscriminatedUnionEncoder {
    pub(crate) encoders: HashMap<String, Box<TEncoder>>,
    pub(crate) dump_discriminator: Py<PyString>,
    pub(crate) load_discriminator: Py<PyString>,
    pub(crate) load_discriminator_rs: String,
    pub(crate) keys: Vec<String>,
    #[allow(dead_code)]
    pub(crate) ctx: Context,
}

impl Encoder for DiscriminatedUnionEncoder {
    #[inline]
    fn dump<'a>(&self, value: &Bound<'a, PyAny>) -> PyResult<Bound<'a, PyAny>> {
        let key = match value.getattr(&self.dump_discriminator) {
            Ok(val) => {
                val.str()?
            }
            Err(_) => {
                return Err(missing_required_property(
                    self.dump_discriminator.bind(value.py()).str()?.to_str()?,
                    &InstancePath::new(),
                ))
            }
        };

        let str_key = key.to_str()?;

        let encoder = self.encoders.get(str_key).ok_or_else(|| {
            let instance_path = InstancePath::new();
            no_encoder_for_discriminator(str_key, &self.keys, &instance_path)
        })?;
        encoder.dump(value)
    }

    #[inline]
    fn load(&self, value: *mut PyObject, instance_path: &InstancePath) -> PyResult<*mut PyObject> {
        let py_value = PyValue::new(value);
        match py_value.as_dict() {
            None => invalid_type!("object", py_value, instance_path),
            Some(dict) => {
                let key = match py_object_get_item(dict.as_ptr(), self.load_discriminator.as_ptr())
                {
                    Ok(val) => {
                        let result = py_str_to_str(val)?;
                        ffi!(Py_DECREF(val));
                        result
                    }
                    Err(_) => {
                        return Err(missing_required_property(
                            &self.load_discriminator_rs,
                            instance_path,
                        ))
                    }
                };
                let encoder = self.encoders.get(key).ok_or_else(|| {
                    let instance_path = instance_path.push(self.load_discriminator_rs.clone());
                    no_encoder_for_discriminator(key, &self.keys, &instance_path)
                })?;
                encoder.load(value, instance_path)
            }
        }
    }
}

#[derive(Debug, Clone)]
pub struct TimeEncoder {
    #[allow(dead_code)]
    pub(crate) ctx: Context,
}

impl Encoder for TimeEncoder {
    #[inline]
    fn dump<'a>(&self, value: &Bound<'a, PyAny>) -> PyResult<Bound<'a, PyAny>> {
        let isoformat = intern!(value.py(), "isoformat");
        value.call_method0(isoformat)
    }

    #[inline]
    fn load(&self, value: *mut PyObject, instance_path: &InstancePath) -> PyResult<*mut PyObject> {
        let py_value = PyValue::new(value);
        if let Some(val) = py_value.as_str() {
            if let Ok(result) = parse_time(val) {
                return Ok(result);
            }
        }
        invalid_type!("time", py_value, instance_path)
    }
}

#[derive(Debug, Clone)]
pub struct DateTimeEncoder {
    #[allow(dead_code)]
    pub(crate) ctx: Context,
}

impl Encoder for DateTimeEncoder {
    #[inline]
    fn dump<'a>(&self, value: &Bound<'a, PyAny>) -> PyResult<Bound<'a, PyAny>> {
        let isoformat = intern!(value.py(), "isoformat");
        value.call_method0(isoformat)
    }

    #[inline]
    fn load(&self, value: *mut PyObject, instance_path: &InstancePath) -> PyResult<*mut PyObject> {
        let py_val = PyValue::new(value);
        if let Some(val) = py_val.as_str() {
            if let Ok(result) = parse_datetime(val) {
                return Ok(result);
            }
        }
        invalid_type!("datetime", py_val, instance_path)
    }
}

#[derive(Debug, Clone)]
pub struct DateEncoder {
    #[allow(dead_code)]
    pub(crate) ctx: Context,
}

impl Encoder for DateEncoder {
    #[inline]
    fn dump<'a>(&self, value: &Bound<'a, PyAny>) -> PyResult<Bound<'a, PyAny>> {
        let date = if let Ok(datetime) = value.downcast::<PyDateTime>() {
            datetime.call_method("date", (), None)?
        } else {
            value.clone()
        };
        let isoformat = intern!(value.py(), "isoformat");
        date.call_method0(isoformat)
    }

    #[inline]
    fn load(&self, value: *mut PyObject, instance_path: &InstancePath) -> PyResult<*mut PyObject> {
        let py_val = PyValue::new(value);
        if let Some(val) = py_val.as_str() {
            if let Ok(result) = parse_date(val) {
                return Ok(result);
            }
        }
        invalid_type!("date", py_val, instance_path)
    }
}

#[derive(Debug)]
pub enum Encoders {
    Entity(EntityEncoder),
    TypedDict(TypedDictEncoder),
}

#[derive(Debug, Clone)]
pub struct LazyEncoder {
    pub(crate) inner: Arc<AtomicRefCell<Option<Encoders>>>,
    #[allow(dead_code)]
    pub(crate) ctx: Context,
}

impl Encoder for LazyEncoder {
    #[inline]
    fn dump<'a>(&self, value: &Bound<'a, PyAny>) -> PyResult<Bound<'a, PyAny>> {
        match self.inner.borrow().as_ref() {
            Some(encoder) => match encoder {
                Encoders::Entity(encoder) => encoder.dump(value),
                Encoders::TypedDict(encoder) => encoder.dump(value),
            },
            None => Err(PyRuntimeError::new_err(
                "[RUST] Invalid recursive encoder".to_string(),
            )),
        }
    }

    #[inline]
    fn load(&self, value: *mut PyObject, instance_path: &InstancePath) -> PyResult<*mut PyObject> {
        match self.inner.borrow().as_ref() {
            Some(encoder) => match encoder {
                Encoders::Entity(encoder) => encoder.load(value, instance_path),
                Encoders::TypedDict(encoder) => encoder.load(value, instance_path),
            },
            None => Err(PyRuntimeError::new_err(
                "[RUST] Invalid recursive encoder".to_string(),
            )),
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
    fn dump<'a>(&self, value: &Bound<'a, PyAny>) -> PyResult<Bound<'a, PyAny>> {
        match self.dump {
            Some(ref dump) => dump.bind(value.py()).call1((value, )),
            None => self.inner.dump(value),
        }
    }

    #[inline]
    fn load(&self, value: *mut PyObject, instance_path: &InstancePath) -> PyResult<*mut PyObject> {
        match self.load {
            Some(ref load) => py_object_call1_make_tuple_or_err(load.as_ptr(), value),
            None => self.inner.load(value, instance_path),
        }
    }
}
