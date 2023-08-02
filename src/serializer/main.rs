use super::encoders::{
    CustomEncoder, DateEncoder, DateTimeEncoder, Encoders, LazyEncoder, TEncoder, TimeEncoder,
    TypedDictEncoder, UnionEncoder,
};

use crate::errors::{ToPyErr, ValidationError};
use crate::python::py_str_to_str;
use atomic_refcell::AtomicRefCell;
use pyo3::prelude::*;
use pyo3::types::{PyDict, PyString};
use pyo3::{AsPyPointer, PyAny, PyResult};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;

use crate::jsonschema;
use crate::python::{get_object_type, is_not_set, Type};

use super::encoders::{
    ArrayEncoder, DecimalEncoder, DictionaryEncoder, EntityEncoder, EnumEncoder, Field,
    NoopEncoder, OptionalEncoder, TupleEncoder, UUIDEncoder,
};

type EncoderStateValue = Arc<AtomicRefCell<Option<Encoders>>>;

#[pyclass]
#[derive(Debug)]
pub struct Serializer {
    pub encoder: Box<TEncoder>,
    schema: jsonschema::JSONSchema,
}

#[pymethods]
impl Serializer {
    #[new]
    fn new(type_info: &PyAny, schema: &PyAny) -> PyResult<Self> {
        let obj_type = get_object_type(type_info)?;
        let mut encoder_state: HashMap<usize, EncoderStateValue> = HashMap::new();
        let schema = jsonschema::compile(schema)?;

        let serializer = Self {
            encoder: get_encoder(type_info.py(), obj_type, &mut encoder_state)?,
            schema,
        };
        Ok(serializer)
    }

    #[inline]
    pub fn dump(&self, value: &PyAny) -> PyResult<Py<PyAny>> {
        unsafe {
            Ok(Py::from_owned_ptr(
                value.py(),
                self.encoder.dump(value.as_ptr())?,
            ))
        }
    }

    #[inline]
    pub fn load(&self, value: &PyAny, validate: bool) -> PyResult<Py<PyAny>> {
        if validate {
            jsonschema::validate_python(&self.schema, value)?;
        }
        unsafe {
            Ok(Py::from_owned_ptr(
                value.py(),
                self.encoder.load(value.as_ptr())?,
            ))
        }
    }

    #[inline]
    pub fn load_json(&self, value: &PyAny, validate: bool) -> PyResult<Py<PyAny>> {
        let string = py_str_to_str(value.as_ptr())?;
        let serde_value: Value = serde_json::from_str(string)
            .map_err(|e| ValidationError::new_err(format!("Invalid JSON string: {}", e)))?;
        if validate {
            jsonschema::validate(value.py(), &self.schema, &serde_value)?;
        }
        unsafe {
            Ok(Py::from_owned_ptr(
                value.py(),
                self.encoder.load_value(serde_value)?,
            ))
        }
    }
}

pub fn get_encoder(
    py: Python<'_>,
    obj_type: Type,
    encoder_state: &mut HashMap<usize, EncoderStateValue>,
) -> PyResult<Box<TEncoder>> {
    let encoder: Box<TEncoder> = match obj_type {
        Type::String(type_info)
        | Type::Integer(type_info)
        | Type::Bytes(type_info)
        | Type::Float(type_info)
        | Type::Boolean(type_info)
        | Type::Any(type_info)
        | Type::LiteralType(type_info) => {
            wrap_with_custom_encoder(py, type_info, Box::new(NoopEncoder))?
        }
        Type::Decimal(type_info) => {
            wrap_with_custom_encoder(py, type_info, Box::new(DecimalEncoder))?
        }
        Type::Optional(type_info) => {
            let inner = get_object_type(type_info.getattr(py, "inner")?.as_ref(py))?;
            let encoder = get_encoder(py, inner, encoder_state)?;
            wrap_with_custom_encoder(py, type_info, Box::new(OptionalEncoder { encoder }))?
        }
        Type::Dictionary(type_info) => {
            let key_type = get_object_type(type_info.getattr(py, "key_type")?.as_ref(py))?;
            let value_type = get_object_type(type_info.getattr(py, "value_type")?.as_ref(py))?;
            let omit_none = type_info.getattr(py, "omit_none")?.is_true(py)?;

            let key_encoder = get_encoder(py, key_type, encoder_state)?;
            let value_encoder = get_encoder(py, value_type, encoder_state)?;

            wrap_with_custom_encoder(
                py,
                type_info,
                Box::new(DictionaryEncoder {
                    key_encoder,
                    value_encoder,
                    omit_none,
                }),
            )?
        }
        Type::Array(type_info) => {
            let item_type = get_object_type(type_info.getattr(py, "item_type")?.as_ref(py))?;
            let encoder = get_encoder(py, item_type, encoder_state)?;

            wrap_with_custom_encoder(py, type_info, Box::new(ArrayEncoder { encoder }))?
        }
        Type::Tuple(type_info) => {
            let mut encoders = vec![];
            for item_type in type_info.getattr(py, "item_types")?.as_ref(py).iter()? {
                let item_type = item_type?;
                let encoder = get_encoder(py, get_object_type(item_type)?, encoder_state)?;
                encoders.push(encoder);
            }
            wrap_with_custom_encoder(py, type_info, Box::new(TupleEncoder { encoders }))?
        }
        Type::UnionType(type_info) => {
            let dump_discriminator_raw = type_info.getattr(py, "dump_discriminator")?;
            let dump_discriminator: &PyString = dump_discriminator_raw.as_ref(py).downcast()?;

            let load_discriminator_raw = type_info.getattr(py, "load_discriminator")?;
            let load_discriminator: &PyString = load_discriminator_raw.as_ref(py).downcast()?;

            let item_types_raw = type_info.getattr(py, "item_types")?;
            let item_types: &PyDict = item_types_raw.as_ref(py).downcast()?;

            let mut encoders = HashMap::new();

            for (key, value) in item_types.iter() {
                let key: &PyString = key.downcast()?;
                let encoder = get_encoder(py, get_object_type(value)?, encoder_state)?;
                encoders.insert(key.to_string_lossy().into(), encoder);
            }

            wrap_with_custom_encoder(
                py,
                type_info,
                Box::new(UnionEncoder {
                    encoders,
                    dump_discriminator: dump_discriminator.into(),
                    load_discriminator: load_discriminator.into(),
                    load_discriminator_rs: load_discriminator.to_string_lossy().into(),
                }),
            )?
        }
        Type::Entity(type_info) => {
            let py_type = type_info.getattr(py, "cls")?;
            let class_fields = type_info.getattr(py, "fields")?;
            let omit_none = type_info.getattr(py, "omit_none")?.is_true(py)?;
            let fields = iterate_on_fields(py, class_fields, encoder_state)?;

            let encoder = EntityEncoder {
                fields,
                omit_none,
                cls: py_type,
            };
            let python_object_id = type_info.as_ptr() as *const _ as usize;
            let val = encoder_state.entry(python_object_id).or_default();
            AtomicRefCell::<Option<Encoders>>::borrow_mut(val)
                .replace(Encoders::Entity(encoder.clone()));
            wrap_with_custom_encoder(py, type_info, Box::new(encoder))?
        }
        Type::TypedDict(type_info) => {
            let class_fields = type_info.getattr(py, "fields")?;
            let omit_none = type_info.getattr(py, "omit_none")?.is_true(py)?;
            let fields = iterate_on_fields(py, class_fields, encoder_state)?;

            let encoder = TypedDictEncoder { fields, omit_none };
            let python_object_id = type_info.as_ptr() as *const _ as usize;
            let val = encoder_state.entry(python_object_id).or_default();
            AtomicRefCell::<Option<Encoders>>::borrow_mut(val)
                .replace(Encoders::TypedDict(encoder.clone()));
            wrap_with_custom_encoder(py, type_info, Box::new(encoder))?
        }
        Type::RecursionHolder(type_info) => {
            let inner_type = type_info.call_method0(py, "get_type")?;
            let python_object_id = inner_type.as_ptr() as *const _ as usize;
            let encoder = encoder_state.entry(python_object_id).or_default();
            wrap_with_custom_encoder(
                py,
                type_info,
                Box::new(LazyEncoder {
                    inner: encoder.clone(),
                }),
            )?
        }
        Type::Uuid(type_info) => wrap_with_custom_encoder(py, type_info, Box::new(UUIDEncoder))?,
        Type::Enum(type_info) => {
            let py_type = type_info.getattr(py, "cls")?;
            wrap_with_custom_encoder(py, type_info, Box::new(EnumEncoder { enum_type: py_type }))?
        }
        Type::DateTime(type_info) => {
            wrap_with_custom_encoder(py, type_info, Box::new(DateTimeEncoder))?
        }
        Type::Time(type_info) => wrap_with_custom_encoder(py, type_info, Box::new(TimeEncoder))?,
        Type::Date(type_info) => wrap_with_custom_encoder(py, type_info, Box::new(DateEncoder))?,
    };

    Ok(encoder)
}

fn wrap_with_custom_encoder(
    py: Python<'_>,
    type_info: Py<PyAny>,
    original_encoder: Box<TEncoder>,
) -> PyResult<Box<TEncoder>> {
    let custom_encoder = type_info.getattr(py, "custom_encoder")?;
    if custom_encoder.is_none(py) {
        return Ok(original_encoder);
    }
    let dump = to_optional(py, custom_encoder.getattr(py, "serialize")?);
    let load = to_optional(py, custom_encoder.getattr(py, "deserialize")?);

    if dump.is_none() && load.is_none() {
        return Ok(original_encoder);
    }

    Ok(Box::new(CustomEncoder {
        inner: original_encoder,
        dump,
        load,
    }))
}

fn to_optional(py: Python<'_>, value: PyObject) -> Option<PyObject> {
    match value.is_none(py) {
        true => None,
        false => Some(value),
    }
}

fn iterate_on_fields(
    py: Python<'_>,
    fields_attr: PyObject,
    encoder_state: &mut HashMap<usize, EncoderStateValue>,
) -> PyResult<Vec<Field>> {
    let mut fields = vec![];
    for field in fields_attr.as_ref(py).iter()? {
        let field = field?;
        let f_name: &PyString = field.getattr("name")?.downcast()?;
        let dict_key: &PyString = field.getattr("dict_key")?.downcast()?;
        let required = field.getattr("required")?.is_true()?;
        let f_type = get_object_type(field.getattr("type")?)?;
        let f_default = field.getattr("default")?;
        let f_default_factory = field.getattr("default_factory")?;

        let fld = Field {
            name: f_name.into(),
            dict_key: dict_key.into(),
            dict_key_rs: dict_key.to_string_lossy().into(),
            encoder: get_encoder(py, f_type, encoder_state)?,
            required,
            default: match is_not_set(f_default)? {
                true => None,
                false => Some(f_default.into()),
            },
            default_factory: match is_not_set(f_default_factory)? {
                true => None,
                false => Some(f_default_factory.into()),
            },
        };
        fields.push(fld);
    }
    Ok(fields)
}
