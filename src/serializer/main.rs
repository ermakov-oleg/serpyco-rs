use std::collections::HashMap;
use std::sync::Arc;

use atomic_refcell::AtomicRefCell;
use pyo3::prelude::*;
use pyo3::types::{PyDict, PyList, PyString};
use pyo3::{intern, PyAny, PyResult};

use crate::python::{get_object_type, Type};
use crate::serializer::encoders::{
    BooleanEncoder, BytesEncoder, FloatEncoder, IntEncoder, LiteralEncoder, StringEncoder,
    TypedDictEncoder, UnionEncoder,
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
        let ctx = Context::new();

        let serializer = Self {
            encoder: get_encoder(type_info.py(), obj_type, &mut encoder_state, ctx)?,
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
        self.encoder.load(value, &instance_path)
    }
}

pub fn get_encoder(
    py: Python<'_>,
    obj_type: Type,
    encoder_state: &mut HashMap<usize, EncoderStateValue>,
    ctx: Context,
) -> PyResult<Box<TEncoder>> {
    let encoder: Box<TEncoder> = match obj_type {
        Type::Integer(type_info, base_type) => {
            let type_info = type_info.get().clone();
            let encoder = IntEncoder { ctx, type_info };
            wrap_with_custom_encoder(py, base_type, Box::new(encoder))?
        }
        Type::String(type_info, base_type) => {
            let type_info = type_info.get().clone();
            let encoder = StringEncoder { type_info, ctx };
            wrap_with_custom_encoder(py, base_type, Box::new(encoder))?
        }
        Type::Float(type_info, base_type) => {
            let type_info = type_info.get().clone();
            let encoder = FloatEncoder { type_info, ctx };
            wrap_with_custom_encoder(py, base_type, Box::new(encoder))?
        }
        Type::Decimal(type_info, base_type) => {
            let type_info = type_info.get().clone();
            let decimal_module = PyModule::import_bound(py, "decimal")?;
            let decimal_cls = decimal_module.getattr("Decimal")?;
            let encoder = DecimalEncoder {
                type_info,
                ctx,
                decimal_cls: decimal_cls.unbind(),
            };
            wrap_with_custom_encoder(py, base_type, Box::new(encoder))?
        }
        Type::Boolean(_, base_type) => {
            let encoder = BooleanEncoder { ctx };
            wrap_with_custom_encoder(py, base_type, Box::new(encoder))?
        }
        Type::Uuid(_, base_type) => {
            let uuid = PyModule::import_bound(py, "uuid")?;
            let uuid_cls = uuid.getattr("UUID")?;

            let encoder = UUIDEncoder {
                ctx,
                uuid_cls: uuid_cls.unbind(),
            };
            wrap_with_custom_encoder(py, base_type, Box::new(encoder))?
        }
        Type::Time(_, base_type) => {
            let encoder = TimeEncoder { ctx };
            wrap_with_custom_encoder(py, base_type, Box::new(encoder))?
        }
        Type::DateTime(_, base_type) => {
            let encoder = DateTimeEncoder { ctx };
            wrap_with_custom_encoder(py, base_type, Box::new(encoder))?
        }
        Type::Date(_, base_type) => {
            let encoder = DateEncoder { ctx };
            wrap_with_custom_encoder(py, base_type, Box::new(encoder))?
        }
        Type::Bytes(_, base_type) => {
            let encoder = BytesEncoder { ctx };
            wrap_with_custom_encoder(py, base_type, Box::new(encoder))?
        }
        Type::Any(_, base_type) => wrap_with_custom_encoder(py, base_type, Box::new(NoopEncoder))?,
        Type::Literal(type_info, base_type) => wrap_with_custom_encoder(
            py,
            base_type,
            Box::new(LiteralEncoder {
                enum_items: type_info.get().enum_items.clone(),
                ctx,
            }),
        )?,
        Type::Optional(type_info, base_type) => {
            let inner = get_object_type(type_info.get().inner.bind(py))?;
            let encoder = get_encoder(py, inner, encoder_state, ctx.clone())?;
            wrap_with_custom_encoder(py, base_type, Box::new(OptionalEncoder { encoder, ctx }))?
        }
        Type::Dictionary(type_info, base_type) => {
            let key_type = get_object_type(type_info.get().key_type.bind(py))?;
            let value_type = get_object_type(type_info.get().value_type.bind(py))?;

            let key_encoder = get_encoder(py, key_type, encoder_state, ctx.clone())?;
            let value_encoder = get_encoder(py, value_type, encoder_state, ctx.clone())?;

            wrap_with_custom_encoder(
                py,
                base_type,
                Box::new(DictionaryEncoder {
                    key_encoder,
                    value_encoder,
                    omit_none: type_info.get().omit_none,
                    ctx,
                }),
            )?
        }
        Type::Array(type_info, base_type) => {
            let item_type = get_object_type(type_info.get().item_type.bind(py))?;
            let encoder = get_encoder(py, item_type, encoder_state, ctx.clone())?;
            wrap_with_custom_encoder(py, base_type, Box::new(ArrayEncoder { encoder, ctx }))?
        }
        Type::Tuple(type_info, base_type) => {
            let mut encoders = vec![];
            for item_type in &type_info.get().item_types {
                let item_type = item_type.bind(py);
                let encoder =
                    get_encoder(py, get_object_type(item_type)?, encoder_state, ctx.clone())?;
                encoders.push(encoder);
            }
            wrap_with_custom_encoder(py, base_type, Box::new(TupleEncoder { encoders, ctx }))?
        }
        Type::Union(type_info, base_type) => {
            let item_types = type_info.get().item_types.bind(py).downcast::<PyList>()?;

            let mut encoders = vec![];

            for value in item_types.iter() {
                let encoder =
                    get_encoder(py, get_object_type(&value)?, encoder_state, ctx.clone())?;
                encoders.push(encoder);
            }

            wrap_with_custom_encoder(
                py,
                base_type,
                Box::new(UnionEncoder {
                    encoders,
                    union_repr: type_info.get().union_repr.clone(),
                    ctx,
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
                let encoder =
                    get_encoder(py, get_object_type(&value)?, encoder_state, ctx.clone())?;
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
                    ctx,
                    keys,
                }),
            )?
        }
        Type::Entity(type_info, base_type, python_object_id) => {
            let type_info = type_info.get();
            let fields = iterate_on_fields(py, &type_info.fields, encoder_state, ctx.clone())?;

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
                ctx,
            };
            let val = encoder_state.entry(python_object_id).or_default();
            AtomicRefCell::<Option<Encoders>>::borrow_mut(val)
                .replace(Encoders::Entity(encoder.clone()));
            wrap_with_custom_encoder(py, base_type, Box::new(encoder))?
        }
        Type::TypedDict(type_info, base_type, python_object_id) => {
            let fields =
                iterate_on_fields(py, &type_info.get().fields, encoder_state, ctx.clone())?;

            let encoder = TypedDictEncoder {
                fields,
                omit_none: type_info.get().omit_none,
                ctx,
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
                    ctx,
                }),
            )?
        }
        Type::Enum(type_info, base_type) => wrap_with_custom_encoder(
            py,
            base_type,
            Box::new(EnumEncoder {
                enum_items: type_info.get().enum_items.clone(),
                ctx,
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
    ctx: Context,
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
            encoder: get_encoder(py, f_type, encoder_state, ctx.clone())?,
            required: field.required,
            default: field.default.clone().into(),
            default_factory: field.default_factory.clone().into(),
        };
        fields.push(fld);
    }
    Ok(fields)
}
