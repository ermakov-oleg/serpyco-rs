use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, OnceLock};

use pyo3::buffer::PyBuffer;
use pyo3::exceptions::{PyKeyError, PyRuntimeError, PyTypeError};
use pyo3::prelude::*;
use pyo3::types::{PyByteArray, PyBytes, PyDict, PyList, PyMapping, PyMemoryView, PyString};
use pyo3::{intern, PyAny, PyResult};

use crate::format::{EncodedKey, Parser, Writer};
use crate::python::{get_object_type, BaseTypeInfo, EntityFieldInfo, Type};
use crate::serde_error::SerdeError;
use crate::serializer::encoders::{
    BooleanEncoder, BytesEncoder, CustomTypeEncoder, DiscriminatorKey, FloatEncoder, IntEncoder,
    LiteralEncoder, NeverEncoder, NoneEncoder, QueryFields, StringEncoder, TypedDictEncoder,
    UnionEncoder,
};
use crate::validator::{Context, InstancePath};

use super::encoders::{
    ArrayEncoder, DecimalEncoder, DictionaryEncoder, EntityEncoder, EnumEncoder, Field,
    NoopEncoder, OptionalEncoder, TupleEncoder, UUIDEncoder,
};
use super::encoders::{
    CustomEncoder, DateEncoder, DateTimeEncoder, DiscriminatedUnionEncoder, LazyEncoder, TEncoder,
    TimeEncoder,
};

type CustomEncoderFns = (Option<Py<PyAny>>, Option<Py<PyAny>>);

#[pyclass(frozen, module = "serpyco_rs")]
#[derive(Debug)]
pub struct Serializer {
    pub(crate) encoder: Box<TEncoder>,
    pub(crate) max_recursion_depth: usize,
    /// Byte length of the last `dump_bytes` output, used to pre-size the next
    /// one's buffer. Relaxed: a stale value only costs a re-grow.
    /// Measured: dropping this costs ~0.5% Ir dumping an 8.5KB github-issue payload (mean of 5 runs); see PR #272.
    last_dump_len: AtomicUsize,
}

/// Floor for the dump buffer, and the slack added on top of the size hint so a
/// payload that grew slightly since the last dump still fits without a re-grow.
const DUMP_BUF_MIN: usize = 1024;

#[pymethods]
impl Serializer {
    #[new]
    fn new(
        type_info: &Bound<'_, PyAny>,
        naive_datetime_to_utc: bool,
        max_recursion_depth: usize,
    ) -> PyResult<Self> {
        let obj_type = get_object_type(type_info)?;
        let mut encoder_state = EncoderState::new();

        let serializer = Self {
            encoder: get_encoder(
                type_info.py(),
                obj_type,
                &mut encoder_state,
                naive_datetime_to_utc,
            )?,
            max_recursion_depth,
            last_dump_len: AtomicUsize::new(0),
        };
        Ok(serializer)
    }

    #[inline]
    pub fn dump<'py>(&'py self, value: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
        let ctx = Context::new(false, self.max_recursion_depth);
        self.encoder
            .dump(value, &ctx)
            .map_err(SerdeError::into_py_err)
    }

    #[inline]
    pub fn load<'py>(&'py self, value: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
        let instance_path = InstancePath::new();
        let ctx = Context::new(false, self.max_recursion_depth);
        self.encoder
            .load(value, &instance_path, &ctx)
            .map_err(SerdeError::into_py_err)
    }

    #[inline]
    pub fn dump_bytes<'py>(
        &self,
        py: Python<'py>,
        value: &Bound<'py, PyAny>,
        format: u8,
    ) -> PyResult<Bound<'py, PyBytes>> {
        let ctx = Context::new(false, self.max_recursion_depth);
        let hint = self.last_dump_len.load(Ordering::Relaxed);
        let mut writer = Writer::with_capacity(format, hint.max(DUMP_BUF_MIN) + hint / 8)?;
        self.encoder
            .dump_format(value, &mut writer, &ctx)
            .map_err(SerdeError::into_py_err)?;
        let out = writer.as_bytes();
        self.last_dump_len.store(out.len(), Ordering::Relaxed);
        Ok(PyBytes::new(py, out))
    }

    #[inline]
    pub fn load_bytes<'py>(
        &self,
        py: Python<'py>,
        data: &Bound<'py, PyAny>,
        format: u8,
    ) -> PyResult<Bound<'py, PyAny>> {
        let buf = InputBuffer::extract(data)?;
        let mut parser = Parser::new(format, buf.as_slice())?;
        let instance_path = InstancePath::new();
        let ctx = Context::new(false, self.max_recursion_depth);
        match self
            .encoder
            .load_format(py, &mut parser, &instance_path, &ctx)
        {
            Ok(value) => {
                parser.finish().map_err(SerdeError::into_py_err)?;
                Ok(value)
            }
            // No finish() on error: a schema error leaves the cursor mid-document, so
            // finish() would mask it with a spurious trailing-garbage DecodeError.
            Err(err) => Err(err.into_py_err()),
        }
    }

    #[inline]
    pub fn load_query_params<'py>(
        &'py self,
        data: &Bound<'py, PyAny>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let instance_path = InstancePath::new();
        let ctx = Context::new(true, self.max_recursion_depth);
        let py = data.py();

        let encoder = if let Some(encoder) = self.encoder.as_container_encoder() {
            encoder
        } else {
            Err(PyRuntimeError::new_err(
                "This type is not deserializable from query params",
            ))?
        };

        let data = data.cast::<PyMapping>()?;
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
                    let field_value = data.call_method1(intern!(py, "getall"), (&key,))?;
                    new_data.set_item(&key, field_value)?;
                }
                new_data.into_any()
            }
            QueryFields::Dict(false) => {
                let new_data = PyDict::from_sequence(&data.items()?.into_any())?;
                new_data.into_any()
            }
        };

        encoder
            .load(&new_data, &instance_path, &ctx)
            .map_err(SerdeError::into_py_err)
    }
}

/// load_bytes input: bytes/str borrowed without copy; bytearray/memoryview copied
/// (safety: the buffer could mutate during parsing).
enum InputBuffer<'a> {
    Borrowed(&'a [u8]),
    Owned(Vec<u8>),
}

impl<'a> InputBuffer<'a> {
    fn extract(data: &'a Bound<'a, PyAny>) -> PyResult<Self> {
        if let Ok(b) = data.cast::<PyBytes>() {
            return Ok(InputBuffer::Borrowed(b.as_bytes()));
        }
        if let Ok(s) = data.cast::<PyString>() {
            return Ok(InputBuffer::Borrowed(s.to_str()?.as_bytes()));
        }
        if let Ok(b) = data.cast::<PyByteArray>() {
            return Ok(InputBuffer::Owned(b.to_vec()));
        }
        if data.cast::<PyMemoryView>().is_ok() {
            let buffer: PyBuffer<u8> = PyBuffer::get(data)?;
            return Ok(InputBuffer::Owned(buffer.to_vec(data.py())?));
        }
        Err(PyTypeError::new_err(
            "expected bytes, bytearray, memoryview or str",
        ))
    }

    fn as_slice(&self) -> &[u8] {
        match self {
            InputBuffer::Borrowed(s) => s,
            InputBuffer::Owned(v) => v,
        }
    }
}

pub fn get_encoder(
    py: Python<'_>,
    obj_type: Type,
    encoder_state: &mut EncoderState,
    naive_datetime_to_utc: bool,
) -> PyResult<Box<TEncoder>> {
    let encoder: Box<TEncoder> = match obj_type {
        Type::None(base_type) => {
            let encoder = NoneEncoder {};
            wrap_with_custom_encoder(py, base_type, Box::new(encoder))?
        }
        Type::Never(base_type) => {
            let encoder = NeverEncoder {};
            wrap_with_custom_encoder(py, base_type, Box::new(encoder))?
        }
        Type::Integer(type_info, base_type) => {
            let encoder = IntEncoder { type_info };
            wrap_with_custom_encoder(py, base_type, Box::new(encoder))?
        }
        Type::String(type_info, base_type) => {
            let encoder = StringEncoder { type_info };
            wrap_with_custom_encoder(py, base_type, Box::new(encoder))?
        }
        Type::Float(type_info, base_type) => {
            let encoder = FloatEncoder { type_info };
            wrap_with_custom_encoder(py, base_type, Box::new(encoder))?
        }
        Type::Decimal(type_info, base_type) => {
            let decimal_module = PyModule::import(py, "decimal")?;
            let decimal_cls = decimal_module.getattr("Decimal")?;
            let encoder = DecimalEncoder {
                type_info,
                decimal_cls: decimal_cls.unbind(),
            };
            wrap_with_custom_encoder(py, base_type, Box::new(encoder))?
        }
        Type::Boolean(base_type) => {
            let encoder = BooleanEncoder {};
            wrap_with_custom_encoder(py, base_type, Box::new(encoder))?
        }
        Type::Uuid(base_type) => {
            let uuid = PyModule::import(py, "uuid")?;
            let uuid_cls = uuid.getattr("UUID")?;

            let encoder = UUIDEncoder {
                uuid_cls: uuid_cls.unbind(),
            };
            wrap_with_custom_encoder(py, base_type, Box::new(encoder))?
        }
        Type::Time(base_type) => {
            let encoder = TimeEncoder {};
            wrap_with_custom_encoder(py, base_type, Box::new(encoder))?
        }
        Type::DateTime(base_type) => {
            let encoder = DateTimeEncoder {
                naive_datetime_to_utc,
            };
            wrap_with_custom_encoder(py, base_type, Box::new(encoder))?
        }
        Type::Date(base_type) => {
            let encoder = DateEncoder {};
            wrap_with_custom_encoder(py, base_type, Box::new(encoder))?
        }
        Type::Bytes(base_type) => {
            let encoder = BytesEncoder {};
            wrap_with_custom_encoder(py, base_type, Box::new(encoder))?
        }
        Type::Any(base_type) => wrap_with_custom_encoder(py, base_type, Box::new(NoopEncoder))?,
        Type::Literal(type_info, base_type) => wrap_with_custom_encoder(
            py,
            base_type,
            Box::new(LiteralEncoder {
                enum_items: type_info.items_repr.as_str().into(),
                load_map: type_info.load_map.clone_ref(py),
                dump_map: type_info.dump_map.clone_ref(py),
            }),
        )?,
        Type::Optional(type_info, base_type, python_object_id) => {
            let inner = get_object_type(type_info.inner.bind(py))?;
            let encoder = OptionalEncoder {
                encoder: get_encoder(py, inner, encoder_state, naive_datetime_to_utc)?,
            };

            encoder_state.create_and_register(py, encoder, base_type, python_object_id)?
        }
        Type::Dictionary(type_info, base_type, python_object_id) => {
            let key_type = get_object_type(type_info.key_type.bind(py))?;
            let value_type = get_object_type(type_info.value_type.bind(py))?;

            // A plain `str` key lets the streaming load path use the parsed key
            // directly, skipping a re-validate + clone per key.
            let key_is_plain_str = matches!(
                &key_type,
                Type::String(string_info, base)
                    if string_info.min_length.is_none()
                        && string_info.max_length.is_none()
                        && base.custom_encoder.is_none()
            );

            let key_encoder = get_encoder(py, key_type, encoder_state, naive_datetime_to_utc)?;
            let value_encoder = get_encoder(py, value_type, encoder_state, naive_datetime_to_utc)?;

            let encoder = DictionaryEncoder {
                key_encoder,
                value_encoder,
                omit_none: type_info.omit_none,
                key_is_plain_str,
            };

            encoder_state.create_and_register(py, encoder, base_type, python_object_id)?
        }
        Type::Array(type_info, base_type, python_object_id) => {
            let item_type = get_object_type(type_info.item_type.bind(py))?;
            let items_encoder = get_encoder(py, item_type, encoder_state, naive_datetime_to_utc)?;

            let encoder = ArrayEncoder {
                encoder: items_encoder,
                min_length: type_info.min_length,
                max_length: type_info.max_length,
            };

            encoder_state.create_and_register(py, encoder, base_type, python_object_id)?
        }
        Type::Tuple(type_info, base_type, python_object_id) => {
            let mut encoders = vec![];
            for item_type in &type_info.item_types {
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

            encoder_state.create_and_register(py, encoder, base_type, python_object_id)?
        }
        Type::Union(type_info, base_type, python_object_id) => {
            let item_types = type_info.item_types.bind(py).cast::<PyList>()?;

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
                repr: type_info.repr.as_str().into(),
            };

            encoder_state.create_and_register(py, encoder, base_type, python_object_id)?
        }
        Type::DiscriminatedUnion(type_info, base_type, python_object_id) => {
            let dump_discriminator = type_info.dump_discriminator.bind(py).cast::<PyString>()?;

            let load_discriminator = type_info.load_discriminator.bind(py).cast::<PyString>()?;

            let item_types = type_info.item_types.bind(py).cast::<PyDict>()?;

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

            encoder_state.create_and_register(py, encoder, base_type, python_object_id)?
        }
        Type::Entity(type_info, base_type, python_object_id) => {
            let fields =
                iterate_on_fields(py, &type_info.fields, encoder_state, naive_datetime_to_utc)?;

            let format_routing = build_format_routing(&fields);
            let has_flatten = fields.iter().any(|f| f.is_flattened);
            // omit_none can only drop entries of optional fields.
            let dump_sized = !type_info.omit_none || fields.iter().all(|f| f.required);

            let encoder = EntityEncoder {
                fields,
                omit_none: type_info.omit_none,
                dump_sized,
                is_frozen: type_info.is_frozen,
                cls: type_info.cls.clone_ref(py),
                used_keys: type_info.used_keys.clone_ref(py),
                format_routing,
                has_flatten,
            };

            encoder_state.create_and_register(py, encoder, base_type, python_object_id)?
        }
        Type::TypedDict(type_info, base_type, python_object_id) => {
            let fields =
                iterate_on_fields(py, &type_info.fields, encoder_state, naive_datetime_to_utc)?;

            let format_routing = build_format_routing(&fields);
            let has_flatten = fields.iter().any(|f| f.is_flattened);
            // A missing NotRequired key skips its entry outright, and omit_none
            // drops optional-None entries; both make the length dynamic.
            let dump_sized = fields.iter().all(|f| f.required);

            let encoder = TypedDictEncoder {
                fields,
                omit_none: type_info.omit_none,
                dump_sized,
                used_keys: type_info.used_keys.clone_ref(py),
                format_routing,
                has_flatten,
            };

            encoder_state.create_and_register(py, encoder, base_type, python_object_id)?
        }
        Type::RecursionHolder(type_info, base_type) => {
            let encoder_ref = encoder_state.get_encoder_ref(type_info.inner_type_id);
            wrap_with_custom_encoder(py, base_type, Box::new(LazyEncoder { inner: encoder_ref }))?
        }
        Type::Enum(type_info, base_type) => wrap_with_custom_encoder(
            py,
            base_type,
            Box::new(EnumEncoder {
                enum_items: type_info.items_repr.as_str().into(),
                load_map: type_info.load_map.clone_ref(py),
                dump_map: type_info.dump_map.clone(),
            }),
        )?,
        Type::Custom(base_type) => {
            let Some(custom_encoder_py) = &base_type.custom_encoder else {
                return Err(PyRuntimeError::new_err(
                    "CustomType must have both serialize and deserialize methods",
                ));
            };
            let (Some(serialize), Some(deserialize)) =
                extract_custom_encoder(py, custom_encoder_py)?
            else {
                return Err(PyRuntimeError::new_err(
                    "CustomType must have both serialize and deserialize methods",
                ));
            };
            Box::new(CustomTypeEncoder {
                dump: serialize,
                load: deserialize,
            })
        }
    };

    Ok(encoder)
}

fn wrap_with_custom_encoder(
    py: Python<'_>,
    base_type: BaseTypeInfo,
    original_encoder: Box<TEncoder>,
) -> PyResult<Box<TEncoder>> {
    if let Some(custom_encoder_py) = &base_type.custom_encoder {
        let (serialize, deserialize) = extract_custom_encoder(py, custom_encoder_py)?;

        if serialize.is_none() && deserialize.is_none() {
            return Ok(original_encoder);
        }

        Ok(Box::new(CustomEncoder {
            inner: original_encoder,
            dump: serialize,
            load: deserialize,
        }))
    } else {
        Ok(original_encoder)
    }
}

fn extract_custom_encoder(
    py: Python<'_>,
    custom_encoder: &Py<PyAny>,
) -> PyResult<CustomEncoderFns> {
    let custom_encoder = custom_encoder.bind(py);
    let serialize = custom_encoder.getattr(intern!(py, "serialize"))?;
    let deserialize = custom_encoder.getattr(intern!(py, "deserialize"))?;

    Ok((
        if serialize.is_none() {
            None
        } else {
            Some(serialize.unbind())
        },
        if deserialize.is_none() {
            None
        } else {
            Some(deserialize.unbind())
        },
    ))
}

/// Maps JSON key -> field index for the streaming load path. Flatten fields are
/// excluded — they consume unclaimed keys, which that path collects separately.
fn build_format_routing(fields: &[Field]) -> rustc_hash::FxHashMap<String, usize> {
    fields
        .iter()
        .enumerate()
        .filter(|(_, f)| !f.is_flattened)
        .map(|(i, f)| (f.dict_key_rs.clone(), i))
        .collect()
}

fn iterate_on_fields(
    py: Python<'_>,
    entity_fields: &Vec<EntityFieldInfo>,
    encoder_state: &mut EncoderState,
    naive_datetime_to_utc: bool,
) -> PyResult<Vec<Field>> {
    let mut fields = vec![];
    for field in entity_fields {
        let f_name = field.name.cast_bound::<PyString>(py)?;
        let dict_key = field.dict_key.cast_bound::<PyString>(py)?;
        let f_type = get_object_type(field.field_type.bind(py))?;

        let dict_key_rs: String = dict_key.to_string_lossy().into();
        let fld = Field {
            name: f_name.clone().unbind(),
            dict_key: dict_key.clone().unbind(),
            dump_key: EncodedKey::new(&dict_key_rs),
            dict_key_rs,
            encoder: get_encoder(py, f_type, encoder_state, naive_datetime_to_utc)?,
            required: field.required,
            default: field.default.as_ref().map(|value| value.clone_ref(py)),
            default_factory: field
                .default_factory
                .as_ref()
                .map(|value| value.clone_ref(py)),
            is_flattened: field.is_flattened,
            is_dict_flatten: field.is_dict_flatten,
        };
        fields.push(fld);
    }
    Ok(fields)
}

type EncoderStateValue = Arc<OnceLock<Arc<TEncoder>>>;

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

    pub fn get_encoder_ref(&mut self, python_object_id: usize) -> EncoderStateValue {
        self.state.entry(python_object_id).or_default().clone()
    }

    pub fn create_and_register<T>(
        &mut self,
        py: Python<'_>,
        encoder: T,
        base_type: BaseTypeInfo,
        python_object_id: usize,
    ) -> PyResult<Box<TEncoder>>
    where
        T: Clone + crate::serializer::encoders::Encoder + Send + Sync + 'static,
    {
        let shared: Arc<TEncoder> = Arc::new(encoder.clone());
        // Encoder graph is built linearly during `Serializer::new`; this slot
        // is filled exactly once. Ignore the duplicate-init error from `set`.
        let _ = self.state.entry(python_object_id).or_default().set(shared);
        wrap_with_custom_encoder(py, base_type, Box::new(encoder))
    }
}
