use std::collections::HashMap;
use std::sync::Arc;

use atomic_refcell::AtomicRefCell;
use pyo3::prelude::*;
use pyo3::types::{PyDict, PyString};
use pyo3::{PyAny, PyResult};

use crate::python::{get_object_type, Type};
use crate::serializer::encoders::{
    BooleanEncoder, BytesEncoder, FloatEncoder, IntEncoder, LiteralEncoder, StringEncoder,
    TypedDictEncoder,
};
use crate::validator::types::{BaseType, EntityField};
use crate::validator::{types, Context, InstancePath};

use super::encoders::{
    ArrayEncoder, DecimalEncoder, DictionaryEncoder, EntityEncoder, EnumEncoder, Field,
    NoopEncoder, OptionalEncoder, TupleEncoder, UUIDEncoder,
};
use super::encoders::{
    CustomEncoder, DateEncoder, DateTimeEncoder, Encoders, LazyEncoder, TEncoder, TimeEncoder,
    UnionEncoder,
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
    fn new(type_info: &PyAny) -> PyResult<Self> {
        let obj_type = get_object_type(type_info)?;
        let mut encoder_state: HashMap<usize, EncoderStateValue> = HashMap::new();
        let ctx = Context::new();

        let serializer = Self {
            encoder: get_encoder(type_info.py(), obj_type, &mut encoder_state, ctx)?,
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
        let instance_path = InstancePath::new();
        unsafe {
            Ok(Py::from_owned_ptr(
                value.py(),
                self.encoder.load(value.as_ptr(), &instance_path)?,
            ))
        }
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
            let encoder = IntEncoder { ctx, type_info };
            wrap_with_custom_encoder(py, base_type, Box::new(encoder))?
        }
        Type::String(type_info, base_type) => {
            let encoder = StringEncoder { type_info, ctx };
            wrap_with_custom_encoder(py, base_type, Box::new(encoder))?
        }
        Type::Float(type_info, base_type) => {
            let encoder = FloatEncoder { type_info, ctx };
            wrap_with_custom_encoder(py, base_type, Box::new(encoder))?
        }
        Type::Decimal(type_info, base_type) => {
            let encoder = DecimalEncoder { type_info, ctx };
            wrap_with_custom_encoder(py, base_type, Box::new(encoder))?
        }
        Type::Boolean(_, base_type) => {
            let encoder = BooleanEncoder { ctx };
            wrap_with_custom_encoder(py, base_type, Box::new(encoder))?
        }
        Type::Uuid(_, base_type) => {
            let encoder = UUIDEncoder { ctx };
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
                enum_items: type_info.enum_items,
                ctx,
            }),
        )?,
        Type::Optional(type_info, base_type) => {
            let inner = get_object_type(type_info.inner.as_ref(py))?;
            let encoder = get_encoder(py, inner, encoder_state, ctx.clone())?;
            wrap_with_custom_encoder(py, base_type, Box::new(OptionalEncoder { encoder, ctx }))?
        }
        Type::Dictionary(type_info, base_type) => {
            let key_type = get_object_type(type_info.key_type.as_ref(py))?;
            let value_type = get_object_type(type_info.value_type.as_ref(py))?;

            let key_encoder = get_encoder(py, key_type, encoder_state, ctx.clone())?;
            let value_encoder = get_encoder(py, value_type, encoder_state, ctx.clone())?;

            wrap_with_custom_encoder(
                py,
                base_type,
                Box::new(DictionaryEncoder {
                    key_encoder,
                    value_encoder,
                    omit_none: type_info.omit_none,
                    ctx,
                }),
            )?
        }
        Type::Array(type_info, base_type) => {
            let item_type = get_object_type(type_info.item_type.as_ref(py))?;
            let encoder = get_encoder(py, item_type, encoder_state, ctx.clone())?;
            wrap_with_custom_encoder(py, base_type, Box::new(ArrayEncoder { encoder, ctx }))?
        }
        Type::Tuple(type_info, base_type) => {
            let mut encoders = vec![];
            for item_type in type_info.item_types {
                let item_type = item_type.as_ref(py);
                let encoder =
                    get_encoder(py, get_object_type(item_type)?, encoder_state, ctx.clone())?;
                encoders.push(encoder);
            }
            wrap_with_custom_encoder(py, base_type, Box::new(TupleEncoder { encoders, ctx }))?
        }
        Type::Union(type_info, base_type) => {
            let dump_discriminator: &PyString =
                type_info.dump_discriminator.as_ref(py).downcast()?;

            let load_discriminator: &PyString =
                type_info.load_discriminator.as_ref(py).downcast()?;

            let item_types: &PyDict = type_info.item_types.as_ref(py).downcast()?;

            let mut encoders = HashMap::new();
            let mut keys = vec![];

            for (key, value) in item_types.iter() {
                let key: &PyString = key.downcast()?;
                let encoder = get_encoder(py, get_object_type(value)?, encoder_state, ctx.clone())?;
                let rs_key: String = key.to_string_lossy().into();
                keys.push(rs_key.clone());
                encoders.insert(rs_key, encoder);
            }

            wrap_with_custom_encoder(
                py,
                base_type,
                Box::new(UnionEncoder {
                    encoders,
                    dump_discriminator: dump_discriminator.into(),
                    load_discriminator: load_discriminator.into(),
                    load_discriminator_rs: load_discriminator.to_string_lossy().into(),
                    ctx,
                    keys,
                }),
            )?
        }
        Type::Entity(type_info, base_type, python_object_id) => {
            let fields = iterate_on_fields(py, type_info.fields, encoder_state, ctx.clone())?;

            let encoder = EntityEncoder {
                fields,
                omit_none: type_info.omit_none,
                cls: type_info.cls,
                ctx,
            };
            let val = encoder_state.entry(python_object_id).or_default();
            AtomicRefCell::<Option<Encoders>>::borrow_mut(val)
                .replace(Encoders::Entity(encoder.clone()));
            wrap_with_custom_encoder(py, base_type, Box::new(encoder))?
        }
        Type::TypedDict(type_info, base_type, python_object_id) => {
            let fields = iterate_on_fields(py, type_info.fields, encoder_state, ctx.clone())?;

            let encoder = TypedDictEncoder {
                fields,
                omit_none: type_info.omit_none,
                ctx,
            };
            let val = encoder_state.entry(python_object_id).or_default();
            AtomicRefCell::<Option<Encoders>>::borrow_mut(val)
                .replace(Encoders::TypedDict(encoder.clone()));
            wrap_with_custom_encoder(py, base_type, Box::new(encoder))?
        }
        Type::RecursionHolder(type_info, base_type) => {
            let inner_type = type_info.get_type(py)?;
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
                enum_items: type_info.enum_items,
                ctx,
            }),
        )?,
    };

    Ok(encoder)
}

fn wrap_with_custom_encoder(
    py: Python<'_>,
    base_type: BaseType,
    original_encoder: Box<TEncoder>,
) -> PyResult<Box<TEncoder>> {
    if let Some(custom_encoder_py) = base_type.custom_encoder {
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
    entity_fields: Vec<EntityField>,
    encoder_state: &mut HashMap<usize, EncoderStateValue>,
    ctx: Context,
) -> PyResult<Vec<Field>> {
    let mut fields = vec![];
    for field in entity_fields {
        let f_name: &PyString = field.name.downcast(py)?;
        let dict_key: &PyString = field.dict_key.downcast(py)?;
        let f_type = get_object_type(field.field_type.as_ref(py))?;

        let fld = Field {
            name: f_name.into(),
            dict_key: dict_key.into(),
            dict_key_rs: dict_key.to_string_lossy().into(),
            encoder: get_encoder(py, f_type, encoder_state, ctx.clone())?,
            required: field.required,
            default: field.default.into(),
            default_factory: field.default_factory.into(),
        };
        fields.push(fld);
    }
    Ok(fields)
}
