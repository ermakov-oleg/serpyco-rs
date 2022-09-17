use pyo3::{PyAny, PyResult};

use crate::serializer::encoders::{
    ArrayEncoder, DecimalEncoder, DictionaryEncoder, Encoder, EntityEncoder, Field, NoopEncoder,
    Serializer,
};
use crate::serializer::py::is_not_set;
use crate::serializer::types::{get_object_type, Type};
use pyo3::prelude::*;
use pyo3::types::{PyString, PyTuple, PyUnicode};

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
        Type::StringType(_)
        | Type::IntegerType(_)
        | Type::BytesType(_)
        | Type::FloatType(_)
        | Type::BooleanType(_)
        | Type::AnyType(_) => Box::new(NoopEncoder),
        Type::DecimalType(_) => Box::new(DecimalEncoder),
        Type::OptionalType(type_info) => {
            let inner = get_object_type(type_info.getattr(py, "inner")?.as_ref(py))?;
            get_encoder(py, inner)?
        }
        Type::DictionaryType(type_info) => {
            let key_type = get_object_type(type_info.getattr(py, "key_type")?.as_ref(py))?;
            let value_type = get_object_type(type_info.getattr(py, "value_type")?.as_ref(py))?;

            let key_encoder = get_encoder(py, key_type)?;
            let value_encoder = get_encoder(py, value_type)?;

            Box::new(DictionaryEncoder {
                key_encoder,
                value_encoder,
            })
        }
        Type::ArrayType(type_info) => {
            let item_type = get_object_type(type_info.getattr(py, "item_type")?.as_ref(py))?;
            let encoder = get_encoder(py, item_type)?;

            Box::new(ArrayEncoder { encoder })
        }
        Type::EntityType(type_info) => {
            let py_type = type_info.getattr(py, "cls")?;
            let class_fields = type_info.getattr(py, "fields")?;
            let mut fields = vec![];

            for field in class_fields.as_ref(py).iter()? {
                let field = field?;
                let f_name: &PyString = field.getattr("name")?.downcast()?;
                let f_type = get_object_type(field.getattr("type")?)?;
                let f_default = field.getattr("default")?;
                let f_default_factory = field.getattr("default_factory")?;

                let fld = Field {
                    name: f_name.into(),
                    dict_key: f_name.into(),
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

            let create_new_object_args = PyTuple::new(py, vec![py_type.clone()]).into();

            Box::new(EntityEncoder {
                create_new_object_args,
                fields,
            })
        }
        t => todo!("add support new types {:?}", t),
    };

    Ok(encoder)
}
