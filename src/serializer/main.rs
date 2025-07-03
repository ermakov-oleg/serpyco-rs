use std::collections::HashMap;
use std::sync::Arc;

use atomic_refcell::AtomicRefCell;
use pyo3::exceptions::{PyKeyError, PyRuntimeError};
use pyo3::prelude::*;
use pyo3::types::{PyDict, PyList, PyMapping, PyString};
use pyo3::{intern, PyAny, PyResult};

use crate::python::{get_object_type, Type};
use crate::serializer::encoders::{
    BooleanEncoder, BytesEncoder, CustomTypeEncoder, DiscriminatorKey, FloatEncoder, IntEncoder,
    LiteralEncoder, NoneEncoder, QueryFields, StringEncoder, TypedDictEncoder, UnionEncoder,
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

#[pyclass(frozen, module = "serde_json")]
#[derive(Debug)]
pub struct Serializer {
    pub encoder: Box<TEncoder>,
}

#[pymethods]
impl Serializer {
    #[new]
    fn new(type_info: &Bound<'_, PyAny>, naive_datetime_to_utc: bool) -> PyResult<Self> {
        let obj_type = get_object_type(type_info)?;
        let mut encoder_state = EncoderState::new();

        let serializer = Self {
            encoder: get_encoder(
                type_info.py(),
                obj_type,
                &mut encoder_state,
                naive_datetime_to_utc,
            )?,
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
                let new_data = PyDict::new(py);
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
                let new_data = PyDict::new(py);
                for key in data.keys()?.iter() {
                    let field_value = data
                        .call_method1(intern!(py, "getall"), (&key,))
                        .expect("Mapping changing during iteration");
                    new_data.set_item(&key, field_value)?;
                }
                new_data.into_any()
            }
            QueryFields::Dict(false) => {
                let new_data = PyDict::from_sequence(&data.items()?.into_any())?;
                new_data.into_any()
            }
        };

        encoder.load(&new_data, &instance_path, &ctx)
    }
}

pub fn get_encoder(
    py: Python<'_>,
    obj_type: Type,
    encoder_state: &mut EncoderState,
    naive_datetime_to_utc: bool,
) -> PyResult<Box<TEncoder>> {
    let encoder: Box<TEncoder> = match obj_type {
        Type::None(_type_info, base_type) => {
            let encoder = NoneEncoder {};
            wrap_with_custom_encoder(py, base_type, Box::new(encoder))?
        }
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
            let decimal_module = PyModule::import(py, "decimal")?;
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
            let uuid = PyModule::import(py, "uuid")?;
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
            let encoder = DateTimeEncoder {
                naive_datetime_to_utc,
            };
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
                enum_items: type_info.get().items_repr.clone(),
                load_map: type_info.get().load_map.clone_ref(py),
                dump_map: type_info.get().dump_map.clone_ref(py),
            }),
        )?,
        Type::Optional(type_info, base_type, python_object_id) => {
            let inner = get_object_type(type_info.get().inner.bind(py))?;
            let encoder = OptionalEncoder {
                encoder: get_encoder(py, inner, encoder_state, naive_datetime_to_utc)?,
            };

            encoder_state.create_and_register(
                py,
                encoder,
                base_type,
                python_object_id,
                Encoders::Optional,
            )?
        }
        Type::Dictionary(type_info, base_type, python_object_id) => {
            let key_type = get_object_type(type_info.get().key_type.bind(py))?;
            let value_type = get_object_type(type_info.get().value_type.bind(py))?;

            let key_encoder = get_encoder(py, key_type, encoder_state, naive_datetime_to_utc)?;
            let value_encoder = get_encoder(py, value_type, encoder_state, naive_datetime_to_utc)?;

            let encoder = DictionaryEncoder {
                key_encoder,
                value_encoder,
                omit_none: type_info.get().omit_none,
            };

            encoder_state.create_and_register(
                py,
                encoder,
                base_type,
                python_object_id,
                Encoders::Dict,
            )?
        }
        Type::Array(type_info, base_type, python_object_id) => {
            let type_info = type_info.get();
            let item_type = get_object_type(type_info.item_type.bind(py))?;
            let items_encoder = get_encoder(py, item_type, encoder_state, naive_datetime_to_utc)?;

            let encoder = ArrayEncoder {
                encoder: items_encoder,
                min_length: type_info.min_length,
                max_length: type_info.max_length,
            };

            encoder_state.create_and_register(
                py,
                encoder,
                base_type,
                python_object_id,
                Encoders::Array,
            )?
        }
        Type::Tuple(type_info, base_type, python_object_id) => {
            let mut encoders = vec![];
            for item_type in &type_info.get().item_types {
                let item_type = item_type.bind(py);
                let encoder = get_encoder(
                    py,
                    get_object_type(item_type)?,
                    encoder_state,
                    naive_datetime_to_utc,
                )?;
                encoders.push(encoder);
            }

            let encoder = TupleEncoder { encoders };

            encoder_state.create_and_register(
                py,
                encoder,
                base_type,
                python_object_id,
                Encoders::Tuple,
            )?
        }
        Type::Union(type_info, base_type, python_object_id) => {
            let item_types = type_info.get().item_types.bind(py).downcast::<PyList>()?;

            let mut encoders = vec![];

            for value in item_types.iter() {
                let encoder = get_encoder(
                    py,
                    get_object_type(&value)?,
                    encoder_state,
                    naive_datetime_to_utc,
                )?;
                encoders.push(encoder);
            }

            let encoder = UnionEncoder {
                encoders,
                repr: type_info.get().repr.clone(),
            };

            encoder_state.create_and_register(
                py,
                encoder,
                base_type,
                python_object_id,
                Encoders::Union,
            )?
        }
        Type::DiscriminatedUnion(type_info, base_type, python_object_id) => {
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
                let key = DiscriminatorKey::try_from(&key).map_err(|e| {
                    PyRuntimeError::new_err(format!("Invalid key for DiscriminatedUnion: {e:?}"))
                })?;
                let encoder = get_encoder(
                    py,
                    get_object_type(&value)?,
                    encoder_state,
                    naive_datetime_to_utc,
                )?;
                keys.push(key.clone());
                encoders.insert(key, encoder);
            }

            let encoder = DiscriminatedUnionEncoder {
                encoders,
                dump_discriminator: dump_discriminator.clone().unbind(),
                load_discriminator: load_discriminator.clone().unbind(),
                load_discriminator_rs: load_discriminator.to_string_lossy().into(),
                keys,
            };

            encoder_state.create_and_register(
                py,
                encoder,
                base_type,
                python_object_id,
                Encoders::DiscriminatedUnion,
            )?
        }
        Type::Entity(type_info, base_type, python_object_id) => {
            let type_info = type_info.get();
            let fields =
                iterate_on_fields(py, &type_info.fields, encoder_state, naive_datetime_to_utc)?;

            let builtins = PyModule::import(py, intern!(py, "builtins"))?;
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

            encoder_state.create_and_register(
                py,
                encoder,
                base_type,
                python_object_id,
                Encoders::Entity,
            )?
        }
        Type::TypedDict(type_info, base_type, python_object_id) => {
            let fields = iterate_on_fields(
                py,
                &type_info.get().fields,
                encoder_state,
                naive_datetime_to_utc,
            )?;

            let encoder = TypedDictEncoder {
                fields,
                omit_none: type_info.get().omit_none,
            };

            encoder_state.create_and_register(
                py,
                encoder,
                base_type,
                python_object_id,
                Encoders::TypedDict,
            )?
        }
        Type::RecursionHolder(type_info, base_type) => {
            let inner_type = type_info.get().get_inner_type(py)?;
            let python_object_id = inner_type.as_ptr() as *const _ as usize;
            let encoder_ref = encoder_state.get_encoder_ref(python_object_id);
            wrap_with_custom_encoder(py, base_type, Box::new(LazyEncoder { inner: encoder_ref }))?
        }
        Type::Enum(type_info, base_type) => wrap_with_custom_encoder(
            py,
            base_type,
            Box::new(EnumEncoder {
                enum_items: type_info.get().items_repr.clone(),
                load_map: type_info.get().load_map.clone_ref(py),
                dump_map: type_info.get().dump_map.clone(),
            }),
        )?,
        Type::Custom(_, base_type) => {
            if let Some(custom_encoder_py) = &base_type.get().custom_encoder {
                let custom_encoder = custom_encoder_py.extract::<types::CustomEncoder>(py)?;

                if custom_encoder.serialize.is_none() || custom_encoder.deserialize.is_none() {
                    return Err(PyRuntimeError::new_err(
                        "CustomType must have both serialize and deserialize methods",
                    ));
                }
                let serialize = custom_encoder.serialize.unwrap();
                let deserialize = custom_encoder.deserialize.unwrap();

                Box::new(CustomTypeEncoder {
                    dump: serialize,
                    load: deserialize,
                })
            } else {
                return Err(PyRuntimeError::new_err(
                    "CustomType must have both serialize and deserialize methods",
                ));
            }
        }
    };

    Ok(encoder)
}

fn wrap_with_custom_encoder(
    py: Python<'_>,
    base_type: Bound<'_, BaseType>,
    original_encoder: Box<TEncoder>,
) -> PyResult<Box<TEncoder>> {
    if let Some(custom_encoder_py) = &base_type.get().custom_encoder {
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
    encoder_state: &mut EncoderState,
    naive_datetime_to_utc: bool,
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
            encoder: get_encoder(py, f_type, encoder_state, naive_datetime_to_utc)?,
            required: field.required,
            default: field.default.clone().into(),
            default_factory: field.default_factory.clone().into(),
        };
        fields.push(fld);
    }
    Ok(fields)
}

type EncoderStateValue = Arc<AtomicRefCell<Option<Encoders>>>;

#[derive(Default)]
pub struct EncoderState {
    state: HashMap<usize, EncoderStateValue>,
}

impl EncoderState {
    pub fn new() -> Self {
        Self {
            state: HashMap::new(),
        }
    }

    fn register_encoder(&mut self, python_object_id: usize, encoder_variant: Encoders) {
        let val = self.state.entry(python_object_id).or_default();
        AtomicRefCell::<Option<Encoders>>::borrow_mut(val).replace(encoder_variant);
    }

    pub fn get_encoder_ref(&mut self, python_object_id: usize) -> EncoderStateValue {
        self.state.entry(python_object_id).or_default().clone()
    }

    pub fn create_and_register<T>(
        &mut self,
        py: Python<'_>,
        encoder: T,
        base_type: Bound<'_, BaseType>,
        python_object_id: usize,
        encoder_variant_fn: impl FnOnce(T) -> Encoders,
    ) -> PyResult<Box<TEncoder>>
    where
        T: Clone + crate::serializer::encoders::Encoder + Send + Sync + 'static,
    {
        self.register_encoder(python_object_id, encoder_variant_fn(encoder.clone()));
        wrap_with_custom_encoder(py, base_type, Box::new(encoder))
    }
}
