use std::collections::HashMap;
use std::sync::Arc;

use atomic_refcell::AtomicRefCell;
use pyo3::exceptions::{PyKeyError, PyRuntimeError};
use pyo3::prelude::*;
use pyo3::types::{PyDict, PyList, PyMapping, PyString};
use pyo3::{intern, PyAny, PyResult};

use crate::python::{get_object_type, Type};
use crate::serializer::encoders::{
    BooleanEncoder, BytesEncoder, FloatEncoder, IntEncoder, LiteralEncoder, QueryFields,
    StringEncoder, TypedDictEncoder, UnionEncoder,
};
use crate::validator::types::{BaseType, EntityField};
use crate::validator::{types, Context, InstancePath};

use super::encoders::{
    ArrayEncoder, DecimalEncoder, DictionaryEncoder, EntityEncoder, EnumEncoder, Field,
    NoopEncoder, OptionalEncoder, TupleEncoder, UUIDEncoder,
};
use super::encoders::{
    CustomEncoder, DateEncoder, DateTimeEncoder, DiscriminatedUnionEncoder, Encoders, LazyEncoder,
    TEncoder, TimeEncoder,
};

type EncoderStateValue = Arc<AtomicRefCell<Option<Encoders>>>;

#[pyclass(frozen, module = "serde_json")]
#[derive(Debug)]
pub struct Serializer {
    pub encoder: Box<TEncoder>,
}

#[pymethods]
impl Serializer {
    #[new]
    fn new(type_info: &Bound<'_, PyAny>) -> PyResult<Self> {
        let obj_type = get_object_type(type_info)?;
        let mut encoder_state: HashMap<usize, EncoderStateValue> = HashMap::new();

        let serializer = Self {
            encoder: get_encoder(type_info.py(), obj_type, &mut encoder_state)?,
        };
        Ok(serializer)
    }

    #[inline]
    pub fn dump<'py>(&'py self, value: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
        self.encoder.dump(value)
    }

    #[inline]
    pub fn load<'py>(&'py self, value: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
        let instance_path = InstancePath::new();
        let ctx = Context::new(false);
        self.encoder.load(value, &instance_path, &ctx)
    }

    #[inline]
    pub fn load_query_params<'py>(
        &'py self,
        data: &Bound<'py, PyAny>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let instance_path = InstancePath::new();
        let ctx = Context::new(true);
        let py = data.py();

        let encoder = if let Some(encoder) = self.encoder.as_container_encoder() {
            encoder
        } else {
            Err(PyRuntimeError::new_err(
                "This type is not deserializable from query params",
            ))?
        };

        let data = data.downcast::<PyMapping>()?;
        let fields = encoder.get_fields();

        let new_data = match fields {
            QueryFields::Object(fields) => {
                let new_data = PyDict::new_bound(py);
                for field in fields {
                    let field_value = if field.is_sequence {
                        data.call_method1(intern!(py, "getall"), (field.name,))
                    } else {
                        data.get_item(field.name)
                    };

                    match field_value {
                        Ok(val) => new_data.set_item(field.name, val)?,
                        Err(e) if e.is_instance_of::<PyKeyError>(py) => {}
                        Err(e) => return Err(e),
                    }
                }
                new_data.into_any()
            }
            QueryFields::Dict(true) => {
                let new_data = PyDict::new_bound(py);
                for key in data.keys()?.iter()? {
                    let key = key?;
                    let field_value = data
                        .call_method1(intern!(py, "getall"), (&key,))
                        .expect("Mapping changing during iteration");
                    new_data.set_item(&key, field_value)?;
                }
                new_data.into_any()
            }
            QueryFields::Dict(false) => {
                let new_data = PyDict::from_sequence_bound(&data.items()?.into_any())?;
                new_data.into_any()
            }
        };

        encoder.load(&new_data, &instance_path, &ctx)
    }
}

pub fn get_encoder(
    py: Python<'_>,
    obj_type: Type,
    encoder_state: &mut HashMap<usize, EncoderStateValue>,
) -> PyResult<Box<TEncoder>> {
    let encoder: Box<TEncoder> = match obj_type {
        Type::Integer(type_info, base_type) => {
            let type_info = type_info.get().clone();
            let encoder = IntEncoder { type_info };
            wrap_with_custom_encoder(py, base_type, Box::new(encoder))?
        }
        Type::String(type_info, base_type) => {
            let type_info = type_info.get().clone();
            let encoder = StringEncoder { type_info };
            wrap_with_custom_encoder(py, base_type, Box::new(encoder))?
        }
        Type::Float(type_info, base_type) => {
            let type_info = type_info.get().clone();
            let encoder = FloatEncoder { type_info };
            wrap_with_custom_encoder(py, base_type, Box::new(encoder))?
        }
        Type::Decimal(type_info, base_type) => {
            let type_info = type_info.get().clone();
            let decimal_module = PyModule::import_bound(py, "decimal")?;
            let decimal_cls = decimal_module.getattr("Decimal")?;
            let encoder = DecimalEncoder {
                type_info,
                decimal_cls: decimal_cls.unbind(),
            };
            wrap_with_custom_encoder(py, base_type, Box::new(encoder))?
        }
        Type::Boolean(_, base_type) => {
            let encoder = BooleanEncoder {};
            wrap_with_custom_encoder(py, base_type, Box::new(encoder))?
        }
        Type::Uuid(_, base_type) => {
            let uuid = PyModule::import_bound(py, "uuid")?;
            let uuid_cls = uuid.getattr("UUID")?;

            let encoder = UUIDEncoder {
                uuid_cls: uuid_cls.unbind(),
            };
            wrap_with_custom_encoder(py, base_type, Box::new(encoder))?
        }
        Type::Time(_, base_type) => {
            let encoder = TimeEncoder {};
            wrap_with_custom_encoder(py, base_type, Box::new(encoder))?
        }
        Type::DateTime(_, base_type) => {
            let encoder = DateTimeEncoder {};
            wrap_with_custom_encoder(py, base_type, Box::new(encoder))?
        }
        Type::Date(_, base_type) => {
            let encoder = DateEncoder {};
            wrap_with_custom_encoder(py, base_type, Box::new(encoder))?
        }
        Type::Bytes(_, base_type) => {
            let encoder = BytesEncoder {};
            wrap_with_custom_encoder(py, base_type, Box::new(encoder))?
        }
        Type::Any(_, base_type) => wrap_with_custom_encoder(py, base_type, Box::new(NoopEncoder))?,
        Type::Literal(type_info, base_type) => wrap_with_custom_encoder(
            py,
            base_type,
            Box::new(LiteralEncoder {
                enum_items: type_info.get().enum_items.clone(),
            }),
        )?,
        Type::Optional(type_info, base_type) => {
            let inner = get_object_type(type_info.get().inner.bind(py))?;
            let encoder = get_encoder(py, inner, encoder_state)?;
            wrap_with_custom_encoder(py, base_type, Box::new(OptionalEncoder { encoder }))?
        }
        Type::Dictionary(type_info, base_type) => {
            let key_type = get_object_type(type_info.get().key_type.bind(py))?;
            let value_type = get_object_type(type_info.get().value_type.bind(py))?;

            let key_encoder = get_encoder(py, key_type, encoder_state)?;
            let value_encoder = get_encoder(py, value_type, encoder_state)?;

            wrap_with_custom_encoder(
                py,
                base_type,
                Box::new(DictionaryEncoder {
                    key_encoder,
                    value_encoder,
                    omit_none: type_info.get().omit_none,
                }),
            )?
        }
        Type::Array(type_info, base_type) => {
            let item_type = get_object_type(type_info.get().item_type.bind(py))?;
            let encoder = get_encoder(py, item_type, encoder_state)?;
            wrap_with_custom_encoder(py, base_type, Box::new(ArrayEncoder { encoder }))?
        }
        Type::Tuple(type_info, base_type) => {
            let mut encoders = vec![];
            for item_type in &type_info.get().item_types {
                let item_type = item_type.bind(py);
                let encoder = get_encoder(py, get_object_type(item_type)?, encoder_state)?;
                encoders.push(encoder);
            }
            wrap_with_custom_encoder(py, base_type, Box::new(TupleEncoder { encoders }))?
        }
        Type::Union(type_info, base_type) => {
            let item_types = type_info.get().item_types.bind(py).downcast::<PyList>()?;

            let mut encoders = vec![];

            for value in item_types.iter() {
                let encoder = get_encoder(py, get_object_type(&value)?, encoder_state)?;
                encoders.push(encoder);
            }

            wrap_with_custom_encoder(
                py,
                base_type,
                Box::new(UnionEncoder {
                    encoders,
                    union_repr: type_info.get().union_repr.clone(),
                }),
            )?
        }
        Type::DiscriminatedUnion(type_info, base_type) => {
            let dump_discriminator = type_info
                .get()
                .dump_discriminator
                .bind(py)
                .downcast::<PyString>()?;

            let load_discriminator = type_info
                .get()
                .load_discriminator
                .bind(py)
                .downcast::<PyString>()?;

            let item_types = type_info.get().item_types.bind(py).downcast::<PyDict>()?;

            let mut encoders = HashMap::new();
            let mut keys = vec![];

            for (key, value) in item_types.iter() {
                let key = key.downcast::<PyString>()?;
                let encoder = get_encoder(py, get_object_type(&value)?, encoder_state)?;
                let rs_key: String = key.to_string_lossy().into();
                keys.push(rs_key.clone());
                encoders.insert(rs_key, encoder);
            }

            wrap_with_custom_encoder(
                py,
                base_type,
                Box::new(DiscriminatedUnionEncoder {
                    encoders,
                    dump_discriminator: dump_discriminator.clone().unbind(),
                    load_discriminator: load_discriminator.clone().unbind(),
                    load_discriminator_rs: load_discriminator.to_string_lossy().into(),
                    keys,
                }),
            )?
        }
        Type::Entity(type_info, base_type, python_object_id) => {
            let type_info = type_info.get();
            let fields = iterate_on_fields(py, &type_info.fields, encoder_state)?;

            let builtins = PyModule::import_bound(py, intern!(py, "builtins"))?;
            let object = builtins.getattr(intern!(py, "object"))?;
            let create_object = object.getattr(intern!(py, "__new__"))?;
            let object_set_attr = object.getattr("__setattr__")?;

            let encoder = EntityEncoder {
                fields,
                omit_none: type_info.omit_none,
                is_frozen: type_info.is_frozen,
                create_object: create_object.unbind(),
                object_set_attr: object_set_attr.unbind(),
                cls: type_info.cls.clone(),
            };
            let val = encoder_state.entry(python_object_id).or_default();
            AtomicRefCell::<Option<Encoders>>::borrow_mut(val)
                .replace(Encoders::Entity(encoder.clone()));
            wrap_with_custom_encoder(py, base_type, Box::new(encoder))?
        }
        Type::TypedDict(type_info, base_type, python_object_id) => {
            let fields = iterate_on_fields(py, &type_info.get().fields, encoder_state)?;

            let encoder = TypedDictEncoder {
                fields,
                omit_none: type_info.get().omit_none,
            };
            let val = encoder_state.entry(python_object_id).or_default();
            AtomicRefCell::<Option<Encoders>>::borrow_mut(val)
                .replace(Encoders::TypedDict(encoder.clone()));
            wrap_with_custom_encoder(py, base_type, Box::new(encoder))?
        }
        Type::RecursionHolder(type_info, base_type) => {
            let inner_type = type_info.get().get_inner_type(py)?;
            let python_object_id = inner_type.as_ptr() as *const _ as usize;
            let encoder = encoder_state.entry(python_object_id).or_default();
            wrap_with_custom_encoder(
                py,
                base_type,
                Box::new(LazyEncoder {
                    inner: encoder.clone(),
                }),
            )?
        }
        Type::Enum(type_info, base_type) => wrap_with_custom_encoder(
            py,
            base_type,
            Box::new(EnumEncoder {
                enum_items: type_info.get().enum_items.clone(),
            }),
        )?,
    };

    Ok(encoder)
}

fn wrap_with_custom_encoder(
    py: Python<'_>,
    base_type: Bound<'_, BaseType>,
    original_encoder: Box<TEncoder>,
) -> PyResult<Box<TEncoder>> {
    if let Some(custom_encoder_py) = base_type.get().custom_encoder.clone() {
        let custom_encoder = custom_encoder_py.extract::<types::CustomEncoder>(py)?;

        if custom_encoder.serialize.is_none() && custom_encoder.deserialize.is_none() {
            return Ok(original_encoder);
        }

        Ok(Box::new(CustomEncoder {
            inner: original_encoder,
            dump: custom_encoder.serialize,
            load: custom_encoder.deserialize,
        }))
    } else {
        Ok(original_encoder)
    }
}

fn iterate_on_fields(
    py: Python<'_>,
    entity_fields: &Vec<EntityField>,
    encoder_state: &mut HashMap<usize, EncoderStateValue>,
) -> PyResult<Vec<Field>> {
    let mut fields = vec![];
    for field in entity_fields {
        let f_name = field.name.downcast_bound::<PyString>(py)?;
        let dict_key = field.dict_key.downcast_bound::<PyString>(py)?;
        let f_type = get_object_type(field.field_type.bind(py))?;

        let fld = Field {
            name: f_name.clone().unbind(),
            dict_key: dict_key.clone().unbind(),
            dict_key_rs: dict_key.to_string_lossy().into(),
            encoder: get_encoder(py, f_type, encoder_state)?,
            required: field.required,
            default: field.default.clone().into(),
            default_factory: field.default_factory.clone().into(),
        };
        fields.push(fld);
    }
    Ok(fields)
}
