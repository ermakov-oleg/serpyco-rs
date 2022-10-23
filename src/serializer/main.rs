use crate::serializer::encoders::{DateEncoder, DateTimeEncoder, TimeEncoder};
use pyo3::prelude::*;
use pyo3::types::{PyString, PyTuple};
use pyo3::{PyAny, PyResult};

use super::py::is_not_set;
use super::types::{get_object_type, Type};

use super::encoders::{
    ArrayEncoder, DecimalEncoder, DictionaryEncoder, Encoder, EntityEncoder, EnumEncoder, Field,
    NoopEncoder, OptionalEncoder, Serializer, TupleEncoder, UUIDEncoder,
};

#[pyfunction]
pub fn make_encoder(type_info: &PyAny) -> PyResult<Serializer> {
    let obj_type = get_object_type(type_info)?;
    let serializer = Serializer {
        encoder: get_encoder(type_info.py(), obj_type)?,
    };
    Ok(serializer)
}

pub fn get_encoder(py: Python<'_>, obj_type: Type) -> PyResult<Box<dyn Encoder + Send>> {
    let encoder: Box<dyn Encoder + Send> = match obj_type {
        Type::String | Type::Integer | Type::Bytes | Type::Float | Type::Boolean | Type::Any => {
            Box::new(NoopEncoder)
        }
        Type::Decimal => Box::new(DecimalEncoder),
        Type::Optional(type_info) => {
            let inner = get_object_type(type_info.getattr(py, "inner")?.as_ref(py))?;
            let encoder = get_encoder(py, inner)?;
            Box::new(OptionalEncoder { encoder })
        }
        Type::Dictionary(type_info) => {
            let key_type = get_object_type(type_info.getattr(py, "key_type")?.as_ref(py))?;
            let value_type = get_object_type(type_info.getattr(py, "value_type")?.as_ref(py))?;

            let key_encoder = get_encoder(py, key_type)?;
            let value_encoder = get_encoder(py, value_type)?;

            Box::new(DictionaryEncoder {
                key_encoder,
                value_encoder,
            })
        }
        Type::Array(type_info) => {
            let item_type = get_object_type(type_info.getattr(py, "item_type")?.as_ref(py))?;
            let encoder = get_encoder(py, item_type)?;

            Box::new(ArrayEncoder { encoder })
        }
        Type::Tuple(type_info) => {
            let mut encoders = vec![];
            for item_type in type_info.getattr(py, "item_types")?.as_ref(py).iter()? {
                let item_type = item_type?;
                let encoder = get_encoder(py, get_object_type(item_type)?)?;
                encoders.push(encoder);
            }
            Box::new(TupleEncoder { encoders })
        }
        Type::Entity(type_info) => {
            let py_type = type_info.getattr(py, "cls")?;
            let class_fields = type_info.getattr(py, "fields")?;
            let mut fields = vec![];

            for field in class_fields.as_ref(py).iter()? {
                let field = field?;
                let f_name: &PyString = field.getattr("name")?.downcast()?;
                let dict_key: &PyString = field.getattr("dict_key")?.downcast()?;
                let f_type = get_object_type(field.getattr("type")?)?;
                let f_default = field.getattr("default")?;
                let f_default_factory = field.getattr("default_factory")?;

                let fld = Field {
                    name: f_name.into(),
                    dict_key: dict_key.into(),
                    encoder: get_encoder(py, f_type)?,
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

            let create_new_object_args = PyTuple::new(py, vec![py_type]).into();

            Box::new(EntityEncoder {
                create_new_object_args,
                fields,
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
