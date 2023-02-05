use crate::serializer::encoders::{
    DateEncoder, DateTimeEncoder, LazyEncoder, TEncoder, TimeEncoder,
};
use atomic_refcell::AtomicRefCell;
use pyo3::prelude::*;
use pyo3::types::PyString;
use pyo3::{AsPyPointer, PyAny, PyResult};
use std::collections::HashMap;
use std::sync::Arc;

use super::py::is_not_set;
use super::types::{get_object_type, Type};

use super::encoders::{
    ArrayEncoder, DecimalEncoder, DictionaryEncoder, EntityEncoder, EnumEncoder, Field,
    NoopEncoder, OptionalEncoder, TupleEncoder, UUIDEncoder,
};

type EncoderStateValue = Arc<AtomicRefCell<Option<EntityEncoder>>>;

#[pyclass]
#[derive(Debug)]
pub struct Serializer {
    pub encoder: Box<TEncoder>,
}

#[pymethods]
impl Serializer {
    #[new]
    fn new(type_info: &PyAny) -> PyResult<Self> {
        let obj_type = get_object_type(type_info)?;
        let mut encoder_state: HashMap<usize, EncoderStateValue> = HashMap::new();
        let serializer = Self {
            encoder: get_encoder(type_info.py(), obj_type, &mut encoder_state)?,
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
    pub fn load(&self, value: &PyAny) -> PyResult<Py<PyAny>> {
        unsafe {
            Ok(Py::from_owned_ptr(
                value.py(),
                self.encoder.load(value.as_ptr())?,
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
        Type::String | Type::Integer | Type::Bytes | Type::Float | Type::Boolean | Type::Any => {
            Box::new(NoopEncoder)
        }
        Type::Decimal => Box::new(DecimalEncoder),
        Type::Optional(type_info) => {
            let inner = get_object_type(type_info.getattr(py, "inner")?.as_ref(py))?;
            let encoder = get_encoder(py, inner, encoder_state)?;
            Box::new(OptionalEncoder { encoder })
        }
        Type::Dictionary(type_info) => {
            let key_type = get_object_type(type_info.getattr(py, "key_type")?.as_ref(py))?;
            let value_type = get_object_type(type_info.getattr(py, "value_type")?.as_ref(py))?;
            let omit_none = type_info.getattr(py, "omit_none")?.is_true(py)?;

            let key_encoder = get_encoder(py, key_type, encoder_state)?;
            let value_encoder = get_encoder(py, value_type, encoder_state)?;

            Box::new(DictionaryEncoder {
                key_encoder,
                value_encoder,
                omit_none,
            })
        }
        Type::Array(type_info) => {
            let item_type = get_object_type(type_info.getattr(py, "item_type")?.as_ref(py))?;
            let encoder = get_encoder(py, item_type, encoder_state)?;

            Box::new(ArrayEncoder { encoder })
        }
        Type::Tuple(type_info) => {
            let mut encoders = vec![];
            for item_type in type_info.getattr(py, "item_types")?.as_ref(py).iter()? {
                let item_type = item_type?;
                let encoder = get_encoder(py, get_object_type(item_type)?, encoder_state)?;
                encoders.push(encoder);
            }
            Box::new(TupleEncoder { encoders })
        }
        Type::Entity(type_info) => {
            let py_type = type_info.getattr(py, "cls")?;
            let class_fields = type_info.getattr(py, "fields")?;
            let omit_none = type_info.getattr(py, "omit_none")?.is_true(py)?;
            let mut fields = vec![];

            for field in class_fields.as_ref(py).iter()? {
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

            let encoder = EntityEncoder {
                fields,
                omit_none,
                cls: py_type,
            };
            let python_object_id = type_info.as_ptr() as *const _ as usize;
            let val = encoder_state.entry(python_object_id).or_default();
            AtomicRefCell::<Option<EntityEncoder>>::borrow_mut(val).replace(encoder.clone());
            Box::new(encoder)
        }
        Type::RecursionHolder(type_info) => {
            let inner_type = type_info.call_method0(py, "get_type")?;
            let python_object_id = inner_type.as_ptr() as *const _ as usize;
            let encoder = encoder_state.entry(python_object_id).or_default();
            Box::new(LazyEncoder {
                inner: encoder.clone(),
            })
        }
        Type::Uuid => Box::new(UUIDEncoder),
        Type::Enum(type_info) => {
            let py_type = type_info.getattr(py, "cls")?;
            Box::new(EnumEncoder { enum_type: py_type })
        }
        Type::DateTime => Box::new(DateTimeEncoder),
        Type::Time => Box::new(TimeEncoder),
        Type::Date => Box::new(DateEncoder),
    };

    Ok(encoder)
}
