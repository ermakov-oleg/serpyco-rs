use std::collections::HashMap;
use std::fmt;
use std::fmt::Debug;
use std::sync::Arc;

use atomic_refcell::AtomicRefCell;
use dyn_clone::{clone_trait_object, DynClone};
use nohash_hasher::IntMap;
use pyo3::exceptions::PyRuntimeError;
use pyo3::types::{
    PyBool, PyBytes, PyDate, PyDateTime, PyDict, PyFloat, PyInt, PyList, PySequence, PySet,
    PyString, PyTime,
};
use pyo3::{intern, Bound, Py, PyAny, PyResult};
use pyo3::{prelude::*, IntoPyObjectExt};
use uuid::Uuid;

use crate::errors::{ToPyErr, ValidationError};
use crate::python::{
    create_py_dict_known_size, create_py_list, create_py_tuple, dump_date, dump_datetime,
    dump_time, parse_date, parse_datetime, parse_time, py_dict_set_item, py_list_get_item,
    py_list_set_item, py_tuple_set_item,
};
use crate::validator::types::{DecimalType, FloatType, IntegerType, StringType};
use crate::validator::validators::{
    check_bounds, check_length, check_sequence_bounds, check_sequence_size, invalid_enum_item,
    invalid_type, invalid_type_dump, missing_required_property, no_encoder_for_discriminator,
    str_as_bool,
};
use crate::validator::{map_py_err_to_schema_validation_error, Context, InstancePath};

pub type TEncoder = dyn Encoder + Send + Sync;

pub trait Encoder: DynClone + Debug {
    fn dump<'a>(&self, value: &Bound<'a, PyAny>) -> PyResult<Bound<'a, PyAny>>;
    fn load<'a>(
        &self,
        value: &Bound<'a, PyAny>,
        instance_path: &InstancePath,
        ctx: &Context,
    ) -> PyResult<Bound<'a, PyAny>>;

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
    fn get_fields(&self) -> QueryFields;
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
    fn load<'a>(
        &self,
        value: &Bound<'a, PyAny>,
        _instance_path: &InstancePath,
        _ctx: &Context,
    ) -> PyResult<Bound<'a, PyAny>> {
        Ok(value.clone())
    }
}

#[derive(Debug, Clone)]
pub struct NoneEncoder;

impl Encoder for NoneEncoder {
    #[inline]
    fn dump<'a>(&self, value: &Bound<'a, PyAny>) -> PyResult<Bound<'a, PyAny>> {
        Ok(value.clone())
    }

    #[inline]
    fn load<'a>(
        &self,
        value: &Bound<'a, PyAny>,
        instance_path: &InstancePath,
        _ctx: &Context,
    ) -> PyResult<Bound<'a, PyAny>> {
        if value.is_none() {
            return Ok(value.clone());
        }
        invalid_type!("None", value, instance_path)
    }
}

#[derive(Debug, Clone)]
pub struct IntEncoder {
    pub(crate) type_info: IntegerType,
}

impl Encoder for IntEncoder {
    #[inline]
    fn dump<'a>(&self, value: &Bound<'a, PyAny>) -> PyResult<Bound<'a, PyAny>> {
        Ok(value.clone())
    }

    #[inline]
    fn load<'a>(
        &self,
        value: &Bound<'a, PyAny>,
        instance_path: &InstancePath,
        ctx: &Context,
    ) -> PyResult<Bound<'a, PyAny>> {
        if let Ok(val) = value.downcast::<PyInt>() {
            check_bounds!(val.extract()?, self.type_info, instance_path)?;
            return Ok(value.clone());
        }
        if ctx.try_cast_from_string {
            if let Ok(val) = value.downcast::<PyString>() {
                if let Ok(val) = val.to_str()?.parse::<i64>() {
                    check_bounds!(val, self.type_info, instance_path)?;
                    return val.into_bound_py_any(value.py());
                }
            }
        }
        invalid_type!("integer", value, instance_path)
    }
}

#[derive(Debug, Clone)]
pub struct FloatEncoder {
    pub(crate) type_info: FloatType,
}

impl Encoder for FloatEncoder {
    #[inline]
    fn dump<'a>(&self, value: &Bound<'a, PyAny>) -> PyResult<Bound<'a, PyAny>> {
        Ok(value.clone())
    }
    #[inline]
    fn load<'a>(
        &self,
        value: &Bound<'a, PyAny>,
        instance_path: &InstancePath,
        ctx: &Context,
    ) -> PyResult<Bound<'a, PyAny>> {
        if let Ok(val) = value.downcast::<PyInt>() {
            check_bounds!(val.extract()?, self.type_info, instance_path)?;
            return Ok(value.clone());
        }
        if let Ok(val) = value.downcast::<PyFloat>() {
            check_bounds!(val.extract()?, self.type_info, instance_path)?;
            return Ok(value.clone());
        }
        if ctx.try_cast_from_string {
            if let Ok(val) = value.downcast::<PyString>() {
                if let Ok(val) = val.to_str()?.parse::<f64>() {
                    check_bounds!(val, self.type_info, instance_path)?;
                    return val.into_bound_py_any(value.py());
                }
            }
        }
        invalid_type!("number", value, instance_path)
    }
}

#[derive(Debug, Clone)]
pub struct DecimalEncoder {
    pub(crate) type_info: DecimalType,
    pub(crate) decimal_cls: Py<PyAny>,
}

impl Encoder for DecimalEncoder {
    #[inline]
    fn dump<'a>(&self, value: &Bound<'a, PyAny>) -> PyResult<Bound<'a, PyAny>> {
        Ok(value.str()?.into_any())
    }

    #[inline]
    fn load<'a>(
        &self,
        value: &Bound<'a, PyAny>,
        instance_path: &InstancePath,
        _ctx: &Context,
    ) -> PyResult<Bound<'a, PyAny>> {
        let valid = if let Ok(val) = value.downcast::<PyFloat>() {
            check_bounds!(val.value(), self.type_info, instance_path)?;
            true
        } else if let Ok(val) = value.downcast::<PyInt>() {
            check_bounds!(val.extract()?, self.type_info, instance_path)?;
            true
        } else if let Ok(val) = value.downcast::<PyString>() {
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
            let str_value = value.str().expect("Failed to convert value to string.");
            self.decimal_cls.bind(value.py()).call1((str_value,))
        } else {
            invalid_type!("decimal", value, instance_path)
        }
    }
}

#[derive(Debug, Clone)]
pub struct StringEncoder {
    pub(crate) type_info: StringType,
}

impl Encoder for StringEncoder {
    #[inline]
    fn dump<'a>(&self, value: &Bound<'a, PyAny>) -> PyResult<Bound<'a, PyAny>> {
        Ok(value.clone())
    }

    #[inline]
    fn load<'a>(
        &self,
        value: &Bound<'a, PyAny>,
        instance_path: &InstancePath,
        _ctx: &Context,
    ) -> PyResult<Bound<'a, PyAny>> {
        if let Ok(val) = value.downcast::<PyString>() {
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
}

#[derive(Debug, Clone)]
pub struct BooleanEncoder {}

impl Encoder for BooleanEncoder {
    #[inline]
    fn dump<'a>(&self, value: &Bound<'a, PyAny>) -> PyResult<Bound<'a, PyAny>> {
        Ok(value.clone())
    }

    #[inline]
    fn load<'a>(
        &self,
        value: &Bound<'a, PyAny>,
        instance_path: &InstancePath,
        ctx: &Context,
    ) -> PyResult<Bound<'a, PyAny>> {
        if let Ok(_val) = value.downcast::<PyBool>() {
            return Ok(value.clone());
        }
        if ctx.try_cast_from_string {
            if let Ok(val) = value.downcast::<PyString>() {
                if let Some(val) = str_as_bool(val.to_str()?) {
                    return val.into_bound_py_any(value.py());
                }
            }
        }

        invalid_type!("boolean", value, instance_path)
    }
}

#[derive(Debug, Clone)]
pub struct BytesEncoder {}

impl Encoder for BytesEncoder {
    #[inline]
    fn dump<'a>(&self, value: &Bound<'a, PyAny>) -> PyResult<Bound<'a, PyAny>> {
        Ok(value.clone())
    }

    #[inline]
    fn load<'a>(
        &self,
        value: &Bound<'a, PyAny>,
        instance_path: &InstancePath,
        _ctx: &Context,
    ) -> PyResult<Bound<'a, PyAny>> {
        if let Ok(_val) = value.downcast::<PyBytes>() {
            Ok(value.clone())
        } else {
            invalid_type!("bytes", value, instance_path)
        }
    }
}

#[derive(Debug, Clone)]
pub struct DictionaryEncoder {
    pub(crate) key_encoder: Box<TEncoder>,
    pub(crate) value_encoder: Box<TEncoder>,
    pub(crate) omit_none: bool,
}

impl Encoder for DictionaryEncoder {
    #[inline]
    fn dump<'a>(&self, value: &Bound<'a, PyAny>) -> PyResult<Bound<'a, PyAny>> {
        if let Ok(dict) = value.downcast::<PyDict>() {
            let result_dict = create_py_dict_known_size(dict.py(), dict.len());
            for (k, v) in dict.iter() {
                let key = self.key_encoder.dump(&k)?;
                let value = self.value_encoder.dump(&v)?;
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
    ) -> PyResult<Bound<'a, PyAny>> {
        if let Ok(val) = value.downcast::<PyDict>() {
            let result_dict = create_py_dict_known_size(val.py(), val.len());
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

    fn as_container_encoder(&self) -> Option<&dyn ContainerEncoder> {
        Some(self)
    }
}

impl ContainerEncoder for DictionaryEncoder {
    fn get_fields(&self) -> QueryFields {
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
    fn load<'a>(
        &self,
        value: &Bound<'a, PyAny>,
        instance_path: &InstancePath,
        ctx: &Context,
    ) -> PyResult<Bound<'a, PyAny>> {
        if let Ok(val) = value.downcast::<PyList>() {
            let size = val.len();
            check_sequence_bounds(
                val,
                size,
                self.min_length,
                self.max_length,
                Some(instance_path),
            )?;
            let result = create_py_list(value.py(), size);

            for index in 0..size {
                let item = py_list_get_item(val, index);
                let instance_path = instance_path.push(index);
                let val = self.encoder.load(&item, &instance_path, ctx)?;
                py_list_set_item(&result, index, val);
            }
            Ok(result.into_any())
        } else {
            invalid_type!("list", value, instance_path)
        }
    }

    fn is_sequence(&self) -> bool {
        true
    }
}

#[derive(Debug, Clone)]
pub struct EntityEncoder {
    pub(crate) cls: Py<PyAny>,
    pub(crate) omit_none: bool,
    pub(crate) is_frozen: bool,
    pub(crate) fields: Vec<Field>,
    pub(crate) create_object: Py<PyAny>,
    pub(crate) object_set_attr: Py<PyAny>,
    pub(crate) used_keys: Py<PySet>,
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
    ) -> PyResult<Bound<'a, PyAny>> {
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
    ) -> PyResult<Bound<'a, PyAny>> {
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
    fn dump<'a>(&self, value: &Bound<'a, PyAny>) -> PyResult<Bound<'a, PyAny>> {
        let dict = create_py_dict_known_size(value.py(), self.fields.len());
        for field in &self.fields {
            let field_val = value.getattr(&field.name)?;
            let dump_result = field.encoder.dump(&field_val)?;
            if field.required || !self.omit_none || !dump_result.is_none() {
                if field.is_flattened {
                    dict.update(dump_result.downcast::<pyo3::types::PyMapping>()?)?;
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
    ) -> PyResult<Bound<'a, PyAny>> {
        let Ok(val) = value.downcast::<PyDict>() else {
            invalid_type!("object", value, instance_path)
        };
        let py_frozen_object_set_attr = self.object_set_attr.bind(value.py());
        let obj = self
            .create_object
            .bind(value.py())
            .call1((self.cls.bind(value.py()),))?;

        for field in &self.fields {
            let val = field.load_value(val, instance_path, ctx, &self.used_keys)?;
            if self.is_frozen {
                py_frozen_object_set_attr.call1((&obj, &field.name, val))?;
            } else {
                obj.setattr(&field.name, val)?;
            };
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
    let remaining_dict = create_py_dict_known_size(val.py(), len);
    for (k, v) in val.iter() {
        if !used_keys_set.contains(&k)? {
            remaining_dict.set_item(k, v)?;
        }
    }
    Ok(remaining_dict)
}

fn get_fields_query(fields: &[Field]) -> QueryFields {
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
    fn get_fields(&self) -> QueryFields {
        get_fields_query(&self.fields)
    }
}

#[derive(Debug, Clone)]
pub struct TypedDictEncoder {
    pub(crate) omit_none: bool,
    pub(crate) fields: Vec<Field>,
    pub(crate) used_keys: Py<PySet>,
}

impl Encoder for TypedDictEncoder {
    #[inline]
    fn dump<'a>(&self, value: &Bound<'a, PyAny>) -> PyResult<Bound<'a, PyAny>> {
        let value = match value.downcast::<PyDict>() {
            Ok(val) => val,
            _ => invalid_type_dump!("dict", value),
        };
        let dict = create_py_dict_known_size(value.py(), self.fields.len());
        for field in &self.fields {
            let field_val = match value.get_item(&field.name) {
                Ok(Some(val)) => val,
                _ => {
                    if field.required {
                        return Err(ValidationError::new_err(format!(
                            "data dictionary is missing required parameter {}",
                            &field.name
                        )));
                    }
                    continue;
                }
            };
            let dump_result = field.encoder.dump(&field_val)?;
            if field.required || !self.omit_none || !dump_result.is_none() {
                if field.is_flattened {
                    dict.update(dump_result.downcast::<pyo3::types::PyMapping>()?)?;
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
    ) -> PyResult<Bound<'a, PyAny>> {
        let Ok(value) = value.downcast::<PyDict>() else {
            invalid_type_dump!("dict", value);
        };
        let dict = create_py_dict_known_size(value.py(), self.fields.len());
        for field in &self.fields {
            let val = field.load_value(value, instance_path, ctx, &self.used_keys)?;
            py_dict_set_item(&dict, field.name.as_ptr(), val)?;
        }
        Ok(dict.into_any())
    }
    fn as_container_encoder(&self) -> Option<&dyn ContainerEncoder> {
        Some(self)
    }
}

impl ContainerEncoder for TypedDictEncoder {
    fn get_fields(&self) -> QueryFields {
        get_fields_query(&self.fields)
    }
}

#[derive(Debug, Clone)]
pub struct UUIDEncoder {
    pub(crate) uuid_cls: Py<PyAny>,
}

impl Encoder for UUIDEncoder {
    #[inline]
    fn dump<'a>(&self, value: &Bound<'a, PyAny>) -> PyResult<Bound<'a, PyAny>> {
        Ok(value.str()?.into_any())
    }

    #[inline]
    fn load<'a>(
        &self,
        value: &Bound<'a, PyAny>,
        instance_path: &InstancePath,
        _ctx: &Context,
    ) -> PyResult<Bound<'a, PyAny>> {
        if let Ok(val) = value.downcast::<PyString>() {
            if Uuid::parse_str(val.to_str()?).is_ok() {
                if let Ok(result) = self.uuid_cls.bind(value.py()).call1((val,)) {
                    return Ok(result);
                }
            }
        }
        invalid_type!("uuid", value, instance_path)
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
    fn dump<'a>(&self, value: &Bound<'a, PyAny>) -> PyResult<Bound<'a, PyAny>> {
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
    ) -> PyResult<Bound<'a, PyAny>> {
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
}

#[derive(Debug, Clone)]
pub struct LiteralEncoder {
    pub(crate) enum_items: String,
    pub(crate) load_map: Py<PyDict>,
    pub(crate) dump_map: Py<PyDict>,
}

impl Encoder for LiteralEncoder {
    #[inline]
    fn dump<'a>(&self, value: &Bound<'a, PyAny>) -> PyResult<Bound<'a, PyAny>> {
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
    ) -> PyResult<Bound<'a, PyAny>> {
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
}

#[derive(Debug, Clone)]
pub struct OptionalEncoder {
    pub(crate) encoder: Box<TEncoder>,
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
    fn load<'a>(
        &self,
        value: &Bound<'a, PyAny>,
        instance_path: &InstancePath,
        ctx: &Context,
    ) -> PyResult<Bound<'a, PyAny>> {
        if value.is_none() {
            Ok(value.clone())
        } else {
            self.encoder.load(value, instance_path, ctx)
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
    fn dump<'a>(&self, value: &Bound<'a, PyAny>) -> PyResult<Bound<'a, PyAny>> {
        if let Ok(seq) = value.downcast::<PySequence>() {
            let seq_len = seq.len()?;
            check_sequence_size(seq, seq_len, self.encoders.len(), None)?;
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
    fn load<'a>(
        &self,
        value: &Bound<'a, PyAny>,
        instance_path: &InstancePath,
        ctx: &Context,
    ) -> PyResult<Bound<'a, PyAny>> {
        // Check sequence is not str
        if let Ok(seq) = value.downcast::<PySequence>() {
            if value.is_instance_of::<PyString>() {
                invalid_type!("sequence", value, instance_path);
            }
            let seq_len = seq.len()?;
            check_sequence_size(seq, seq_len, self.encoders.len(), Some(instance_path))?;
            let result = create_py_tuple(value.py(), seq_len);
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
    fn dump<'a>(&self, value: &Bound<'a, PyAny>) -> PyResult<Bound<'a, PyAny>> {
        for encoder in &self.encoders {
            let result = encoder.dump(value);
            if result.is_ok() {
                return result;
            }
        }
        invalid_type_dump!(&self.repr, value)
    }

    #[inline]
    fn load<'a>(
        &self,
        value: &Bound<'a, PyAny>,
        instance_path: &InstancePath,
        ctx: &Context,
    ) -> PyResult<Bound<'a, PyAny>> {
        for encoder in &self.encoders {
            let result = encoder.load(value, instance_path, ctx);
            if result.is_ok() {
                return result;
            }
        }
        invalid_type!(&self.repr, value, instance_path)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct DiscriminatorKey(String);

impl TryFrom<&Bound<'_, PyAny>> for DiscriminatorKey {
    type Error = ();

    fn try_from(value: &Bound<'_, PyAny>) -> Result<Self, Self::Error> {
        if let Ok(val) = value.downcast::<PyString>() {
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
    fn dump<'a>(&self, value: &Bound<'a, PyAny>) -> PyResult<Bound<'a, PyAny>> {
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
        encoder.dump(value)
    }

    #[inline]
    fn load<'a>(
        &self,
        value: &Bound<'a, PyAny>,
        instance_path: &InstancePath,
        ctx: &Context,
    ) -> PyResult<Bound<'a, PyAny>> {
        if let Ok(val) = value.downcast::<PyDict>() {
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
}

#[derive(Debug, Clone)]
pub struct TimeEncoder {}

impl Encoder for TimeEncoder {
    #[inline]
    fn dump<'a>(&self, value: &Bound<'a, PyAny>) -> PyResult<Bound<'a, PyAny>> {
        let py_time = value.downcast::<PyTime>()?;
        let result = dump_time(py_time)?;
        result.into_bound_py_any(value.py())
    }

    #[inline]
    fn load<'a>(
        &self,
        value: &Bound<'a, PyAny>,
        instance_path: &InstancePath,
        _ctx: &Context,
    ) -> PyResult<Bound<'a, PyAny>> {
        if let Ok(val) = value.downcast::<PyString>() {
            if let Ok(result) = parse_time(value.py(), val.to_str()?) {
                return Ok(result.into_any());
            }
        }
        invalid_type!("time", value, instance_path)
    }
}

#[derive(Debug, Clone)]
pub struct DateTimeEncoder {
    pub(crate) naive_datetime_to_utc: bool,
}

impl Encoder for DateTimeEncoder {
    #[inline]
    fn dump<'a>(&self, value: &Bound<'a, PyAny>) -> PyResult<Bound<'a, PyAny>> {
        let py_datetime = value.downcast::<PyDateTime>()?;
        let result = dump_datetime(py_datetime, self.naive_datetime_to_utc)?;
        result.into_bound_py_any(value.py())
    }

    #[inline]
    fn load<'a>(
        &self,
        value: &Bound<'a, PyAny>,
        instance_path: &InstancePath,
        _ctx: &Context,
    ) -> PyResult<Bound<'a, PyAny>> {
        if let Ok(val) = value.downcast::<PyString>() {
            if let Ok(result) = parse_datetime(value.py(), val.to_str()?) {
                return Ok(result.into_any());
            }
        }
        invalid_type!("datetime", value, instance_path)
    }
}

#[derive(Debug, Clone)]
pub struct DateEncoder {}

impl Encoder for DateEncoder {
    #[inline]
    fn dump<'a>(&self, value: &Bound<'a, PyAny>) -> PyResult<Bound<'a, PyAny>> {
        let py_date = value.downcast::<PyDate>()?;
        let result = dump_date(py_date);
        result.into_bound_py_any(value.py())
    }

    #[inline]
    fn load<'a>(
        &self,
        value: &Bound<'a, PyAny>,
        instance_path: &InstancePath,
        _ctx: &Context,
    ) -> PyResult<Bound<'a, PyAny>> {
        if let Ok(val) = value.downcast::<PyString>() {
            if let Ok(result) = parse_date(value.py(), val.to_str()?) {
                return Ok(result.into_any());
            }
        }
        invalid_type!("date", value, instance_path)
    }
}

#[derive(Debug)]
pub enum Encoders {
    Entity(EntityEncoder),
    TypedDict(TypedDictEncoder),
    Dict(DictionaryEncoder),
    Union(UnionEncoder),
    DiscriminatedUnion(DiscriminatedUnionEncoder),
    Tuple(TupleEncoder),
    Array(ArrayEncoder),
    Optional(OptionalEncoder),
}

#[derive(Debug, Clone)]
pub struct LazyEncoder {
    pub(crate) inner: Arc<AtomicRefCell<Option<Encoders>>>,
}

impl Encoder for LazyEncoder {
    #[inline]
    fn dump<'a>(&self, value: &Bound<'a, PyAny>) -> PyResult<Bound<'a, PyAny>> {
        match self.inner.borrow().as_ref() {
            Some(encoder) => match encoder {
                Encoders::Entity(encoder) => encoder.dump(value),
                Encoders::TypedDict(encoder) => encoder.dump(value),
                Encoders::Union(encoder) => encoder.dump(value),
                Encoders::DiscriminatedUnion(encoder) => encoder.dump(value),
                Encoders::Tuple(encoder) => encoder.dump(value),
                Encoders::Array(encoder) => encoder.dump(value),
                Encoders::Optional(encoder) => encoder.dump(value),
                Encoders::Dict(encoder) => encoder.dump(value),
            },
            None => Err(PyRuntimeError::new_err(
                "[RUST] Invalid recursive encoder".to_string(),
            )),
        }
    }

    #[inline]
    fn load<'a>(
        &self,
        value: &Bound<'a, PyAny>,
        instance_path: &InstancePath,
        ctx: &Context,
    ) -> PyResult<Bound<'a, PyAny>> {
        match self.inner.borrow().as_ref() {
            Some(encoder) => match encoder {
                Encoders::Entity(encoder) => encoder.load(value, instance_path, ctx),
                Encoders::TypedDict(encoder) => encoder.load(value, instance_path, ctx),
                Encoders::Tuple(encoder) => encoder.load(value, instance_path, ctx),
                Encoders::Array(encoder) => encoder.load(value, instance_path, ctx),
                Encoders::Optional(encoder) => encoder.load(value, instance_path, ctx),
                Encoders::Union(encoder) => encoder.load(value, instance_path, ctx),
                Encoders::DiscriminatedUnion(encoder) => encoder.load(value, instance_path, ctx),
                Encoders::Dict(encoder) => encoder.load(value, instance_path, ctx),
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
            Some(ref dump) => dump.bind(value.py()).call1((value,)),
            None => self.inner.dump(value),
        }
    }

    #[inline]
    fn load<'a>(
        &self,
        value: &Bound<'a, PyAny>,
        instance_path: &InstancePath,
        ctx: &Context,
    ) -> PyResult<Bound<'a, PyAny>> {
        match self.load {
            Some(ref load) => load.bind(value.py()).call1((value,)).map_err(|err| {
                map_py_err_to_schema_validation_error(value.py(), err, instance_path)
            }),
            None => self.inner.load(value, instance_path, ctx),
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
    fn dump<'a>(&self, value: &Bound<'a, PyAny>) -> PyResult<Bound<'a, PyAny>> {
        self.dump.bind(value.py()).call1((value,))
    }

    #[inline]
    fn load<'a>(
        &self,
        value: &Bound<'a, PyAny>,
        instance_path: &InstancePath,
        _ctx: &Context,
    ) -> PyResult<Bound<'a, PyAny>> {
        let result = self.load.bind(value.py()).call1((value,));
        result.map_err(|err| map_py_err_to_schema_validation_error(value.py(), err, instance_path))
    }
}
