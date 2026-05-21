use std::fmt;

use nohash_hasher::IntMap;
use pyo3::exceptions::PyRuntimeError;
use pyo3::prelude::{PyAnyMethods, PyModule};
use pyo3::sync::PyOnceLock;
use pyo3::types::{PyDict, PyInt, PyList, PyListMethods, PySet, PyType};
use pyo3::{intern, Bound, Py, PyAny, PyResult, Python};

use crate::python::fmt_py;

/// Declarative registry of all supported type-info variants.
///
/// One row generates one variant of `enum TypeTag`, one entry of
/// `TYPE_MAPPINGS` (Python class name → tag), and one arm of the
/// `tag → Type` builder. `enum Type` itself is kept declared by hand directly
/// below, because `macro_rules!` cannot generate enum variants that vary in
/// shape from a flat list. Variant shapes used in the builder arms:
/// - `Unit`                       — `Type::$name(base_type)`
/// - `Info(I, extract_fn)`        — `Type::$name(extract(type_info)?, base_type)`
/// - `CachedInfo(I, extract_fn)`  — `Type::$name(extract(type_info)?, base_type, python_object_id)`
///
/// Adding a new type now requires:
///   1) a row in this macro,
///   2) the matching variant on `enum Type` below,
///   3) the corresponding Python `*Type` dataclass in `_type_info.py`.
macro_rules! type_registry {
    ( $( $name:ident : $cls:literal => $kind:ident $( ( $info:ty , $extract:path ) )? ),* $(,)? ) => {
        #[derive(Copy, Clone, Debug)]
        enum TypeTag {
            $( $name, )*
        }

        const TYPE_MAPPINGS: &[(&str, TypeTag)] = &[
            $( ($cls, TypeTag::$name), )*
        ];

        fn build_type(
            tag: TypeTag,
            type_info: &Bound<'_, PyAny>,
            base_type: BaseTypeInfo,
            python_object_id: usize,
        ) -> PyResult<Type> {
            Ok(match tag {
                $(
                    TypeTag::$name => type_registry!(
                        @build $name $kind base_type type_info python_object_id $($extract)?
                    ),
                )*
            })
        }
    };

    (@build $name:ident Unit $base:ident $ti:ident $pid:ident) => {
        Type::$name($base)
    };
    (@build $name:ident Info $base:ident $ti:ident $pid:ident $extract:path) => {
        Type::$name($extract($ti)?, $base)
    };
    (@build $name:ident CachedInfo $base:ident $ti:ident $pid:ident $extract:path) => {
        Type::$name($extract($ti)?, $base, $pid)
    };
}

#[derive(Clone)]
pub struct BaseTypeInfo {
    pub custom_encoder: Option<Py<PyAny>>,
}

impl fmt::Debug for BaseTypeInfo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("BaseTypeInfo")
            .field("custom_encoder", &self.custom_encoder.is_some())
            .finish()
    }
}

impl BaseTypeInfo {
    fn extract(type_info: &Bound<'_, PyAny>) -> PyResult<Self> {
        let custom_encoder = type_info.getattr("custom_encoder")?;
        Ok(Self {
            custom_encoder: py_none_to_option(custom_encoder),
        })
    }
}

#[derive(Clone, Copy, Debug)]
pub struct NumberTypeInfo<T> {
    pub min: Option<T>,
    pub max: Option<T>,
    pub inclusive_min: bool,
    pub inclusive_max: bool,
}

pub type IntegerTypeInfo = NumberTypeInfo<i64>;
pub type FloatTypeInfo = NumberTypeInfo<f64>;
pub type DecimalTypeInfo = NumberTypeInfo<f64>;

fn extract_i64_number_type(type_info: &Bound<'_, PyAny>) -> PyResult<IntegerTypeInfo> {
    Ok(IntegerTypeInfo {
        min: type_info.getattr("min")?.extract()?,
        max: type_info.getattr("max")?.extract()?,
        inclusive_min: type_info.getattr("inclusive_min")?.extract()?,
        inclusive_max: type_info.getattr("inclusive_max")?.extract()?,
    })
}

fn extract_f64_number_type(type_info: &Bound<'_, PyAny>) -> PyResult<FloatTypeInfo> {
    Ok(FloatTypeInfo {
        min: type_info.getattr("min")?.extract()?,
        max: type_info.getattr("max")?.extract()?,
        inclusive_min: type_info.getattr("inclusive_min")?.extract()?,
        inclusive_max: type_info.getattr("inclusive_max")?.extract()?,
    })
}

#[derive(Clone, Copy, Debug)]
pub struct StringTypeInfo {
    pub min_length: Option<usize>,
    pub max_length: Option<usize>,
}

impl StringTypeInfo {
    fn extract(type_info: &Bound<'_, PyAny>) -> PyResult<Self> {
        Ok(Self {
            min_length: type_info.getattr("min_length")?.extract()?,
            max_length: type_info.getattr("max_length")?.extract()?,
        })
    }
}

#[derive(Clone, Debug)]
pub struct EntityFieldInfo {
    pub name: Py<PyAny>,
    pub dict_key: Py<PyAny>,
    pub field_type: Py<PyAny>,
    pub required: bool,
    pub default: Option<Py<PyAny>>,
    pub default_factory: Option<Py<PyAny>>,
    pub is_flattened: bool,
    pub is_dict_flatten: bool,
}

impl EntityFieldInfo {
    fn extract(field: Bound<'_, PyAny>) -> PyResult<Self> {
        let default = extract_default_value(field.getattr("default")?)?;
        let default_factory = extract_default_value(field.getattr("default_factory")?)?;
        Ok(Self {
            name: field.getattr("name")?.unbind(),
            dict_key: field.getattr("dict_key")?.unbind(),
            field_type: field.getattr("field_type")?.unbind(),
            required: field.getattr("required")?.extract()?,
            default,
            default_factory,
            is_flattened: field.getattr("is_flattened")?.extract()?,
            is_dict_flatten: field.getattr("is_dict_flatten")?.extract()?,
        })
    }
}

#[derive(Clone, Debug)]
pub struct EntityTypeInfo {
    pub cls: Py<PyAny>,
    pub fields: Vec<EntityFieldInfo>,
    pub omit_none: bool,
    pub is_frozen: bool,
    pub used_keys: Py<PySet>,
}

impl EntityTypeInfo {
    fn extract(type_info: &Bound<'_, PyAny>) -> PyResult<Self> {
        Ok(Self {
            cls: type_info.getattr("cls")?.unbind(),
            fields: extract_fields(type_info.getattr("fields")?)?,
            omit_none: type_info.getattr("omit_none")?.extract()?,
            is_frozen: type_info.getattr("is_frozen")?.extract()?,
            used_keys: extract_used_keys(type_info.py(), type_info.getattr("used_keys")?)?,
        })
    }
}

#[derive(Clone, Debug)]
pub struct TypedDictTypeInfo {
    pub fields: Vec<EntityFieldInfo>,
    pub omit_none: bool,
    pub used_keys: Py<PySet>,
}

impl TypedDictTypeInfo {
    fn extract(type_info: &Bound<'_, PyAny>) -> PyResult<Self> {
        Ok(Self {
            fields: extract_fields(type_info.getattr("fields")?)?,
            omit_none: type_info.getattr("omit_none")?.extract()?,
            used_keys: extract_used_keys(type_info.py(), type_info.getattr("used_keys")?)?,
        })
    }
}

#[derive(Clone, Debug)]
pub struct ArrayTypeInfo {
    pub item_type: Py<PyAny>,
    pub min_length: Option<usize>,
    pub max_length: Option<usize>,
}

impl ArrayTypeInfo {
    fn extract(type_info: &Bound<'_, PyAny>) -> PyResult<Self> {
        Ok(Self {
            item_type: type_info.getattr("item_type")?.unbind(),
            min_length: type_info.getattr("min_length")?.extract()?,
            max_length: type_info.getattr("max_length")?.extract()?,
        })
    }
}

#[derive(Clone, Debug)]
pub struct OptionalTypeInfo {
    pub inner: Py<PyAny>,
}

impl OptionalTypeInfo {
    fn extract(type_info: &Bound<'_, PyAny>) -> PyResult<Self> {
        Ok(Self {
            inner: type_info.getattr("inner")?.unbind(),
        })
    }
}

#[derive(Clone, Debug)]
pub struct DictionaryTypeInfo {
    pub key_type: Py<PyAny>,
    pub value_type: Py<PyAny>,
    pub omit_none: bool,
}

impl DictionaryTypeInfo {
    fn extract(type_info: &Bound<'_, PyAny>) -> PyResult<Self> {
        Ok(Self {
            key_type: type_info.getattr("key_type")?.unbind(),
            value_type: type_info.getattr("value_type")?.unbind(),
            omit_none: type_info.getattr("omit_none")?.extract()?,
        })
    }
}

#[derive(Clone, Debug)]
pub struct TupleTypeInfo {
    pub item_types: Vec<Py<PyAny>>,
}

impl TupleTypeInfo {
    fn extract(type_info: &Bound<'_, PyAny>) -> PyResult<Self> {
        Ok(Self {
            item_types: extract_py_list(type_info.getattr("item_types")?)?,
        })
    }
}

#[derive(Clone, Debug)]
pub struct UnionTypeInfo {
    pub item_types: Py<PyAny>,
    pub repr: String,
}

impl UnionTypeInfo {
    fn extract(type_info: &Bound<'_, PyAny>) -> PyResult<Self> {
        Ok(Self {
            item_types: type_info.getattr("item_types")?.unbind(),
            repr: type_info.getattr("repr")?.extract()?,
        })
    }
}

#[derive(Clone, Debug)]
pub struct DiscriminatedUnionTypeInfo {
    pub item_types: Py<PyAny>,
    pub dump_discriminator: Py<PyAny>,
    pub load_discriminator: Py<PyAny>,
}

impl DiscriminatedUnionTypeInfo {
    fn extract(type_info: &Bound<'_, PyAny>) -> PyResult<Self> {
        Ok(Self {
            item_types: type_info.getattr("item_types")?.unbind(),
            dump_discriminator: type_info.getattr("dump_discriminator")?.unbind(),
            load_discriminator: type_info.getattr("load_discriminator")?.unbind(),
        })
    }
}

#[derive(Clone, Debug)]
pub struct EnumTypeInfo {
    pub items_repr: String,
    pub load_map: Py<PyDict>,
    pub dump_map: IntMap<usize, Py<PyAny>>,
}

impl EnumTypeInfo {
    fn extract(type_info: &Bound<'_, PyAny>) -> PyResult<Self> {
        let items = type_info.getattr("items")?;
        let items = items.cast::<PyList>()?;
        let load_map = PyDict::new(type_info.py());
        let mut dump_map = IntMap::default();
        let mut items_repr = Vec::with_capacity(items.len());

        for py_value in items.iter() {
            let value = py_value.getattr(intern!(py_value.py(), "value"))?;
            let py_value_id = py_value.as_ptr() as *const _ as usize;
            dump_map.insert(py_value_id, value.clone().unbind());
            load_map.set_item(&value, &py_value)?;
            items_repr.push(fmt_py(&value));

            if let Ok(value) = value.cast::<PyInt>() {
                let str_value = value.str()?;
                load_map.set_item((&str_value, false), &py_value)?;
            }
        }

        Ok(Self {
            items_repr: format!("[{}]", items_repr.join(", ")),
            load_map: load_map.unbind(),
            dump_map,
        })
    }
}

#[derive(Clone, Debug)]
pub struct LiteralTypeInfo {
    pub items_repr: String,
    pub load_map: Py<PyDict>,
    pub dump_map: Py<PyDict>,
}

impl LiteralTypeInfo {
    fn extract(type_info: &Bound<'_, PyAny>) -> PyResult<Self> {
        let args = type_info.getattr("args")?;
        let args = args.cast::<PyList>()?;
        let load_map = PyDict::new(type_info.py());
        let dump_map = PyDict::new(type_info.py());
        let mut items_repr = Vec::with_capacity(args.len());

        for py_value in args.iter() {
            let value = match py_value.getattr(intern!(py_value.py(), "value")) {
                Ok(value) => value,
                Err(_) => py_value.clone(),
            };

            dump_map.set_item(&py_value, value.clone().unbind())?;
            load_map.set_item(&value, &py_value)?;
            items_repr.push(fmt_py(&value));

            if let Ok(value) = value.cast::<PyInt>() {
                let str_value = value.str()?;
                load_map.set_item((&str_value, false), &py_value)?;
            }
        }

        Ok(Self {
            items_repr: format!("[{}]", items_repr.join(", ")),
            load_map: load_map.unbind(),
            dump_map: dump_map.unbind(),
        })
    }
}

#[derive(Clone, Debug)]
pub struct RecursionHolderInfo {
    pub inner_type_id: usize,
}

impl RecursionHolderInfo {
    fn extract(type_info: &Bound<'_, PyAny>) -> PyResult<Self> {
        let state_key = type_info.getattr("state_key")?;
        let meta = type_info.getattr("meta")?;
        let inner = match meta.get_item(&state_key) {
            Ok(value) if !value.is_none() => value,
            Ok(_) => {
                return Err(PyRuntimeError::new_err(format!(
                    "Recursive type not resolved: {}",
                    fmt_py(&state_key)
                )));
            }
            Err(e) => {
                return Err(PyRuntimeError::new_err(format!(
                    "Recursive type not resolved: {e}"
                )));
            }
        };
        Ok(Self {
            inner_type_id: inner.as_ptr() as *const _ as usize,
        })
    }
}

#[derive(Clone, Debug)]
pub enum Type {
    None(BaseTypeInfo),
    Never(BaseTypeInfo),
    Integer(IntegerTypeInfo, BaseTypeInfo),
    Float(FloatTypeInfo, BaseTypeInfo),
    Decimal(DecimalTypeInfo, BaseTypeInfo),
    String(StringTypeInfo, BaseTypeInfo),
    Boolean(BaseTypeInfo),
    Uuid(BaseTypeInfo),
    Bytes(BaseTypeInfo),
    Time(BaseTypeInfo),
    DateTime(BaseTypeInfo),
    Date(BaseTypeInfo),
    Entity(EntityTypeInfo, BaseTypeInfo, usize),
    TypedDict(TypedDictTypeInfo, BaseTypeInfo, usize),
    Array(ArrayTypeInfo, BaseTypeInfo, usize),
    Enum(EnumTypeInfo, BaseTypeInfo),
    Optional(OptionalTypeInfo, BaseTypeInfo, usize),
    Dictionary(DictionaryTypeInfo, BaseTypeInfo, usize),
    Tuple(TupleTypeInfo, BaseTypeInfo, usize),
    DiscriminatedUnion(DiscriminatedUnionTypeInfo, BaseTypeInfo, usize),
    Union(UnionTypeInfo, BaseTypeInfo, usize),
    Literal(LiteralTypeInfo, BaseTypeInfo),
    Any(BaseTypeInfo),
    RecursionHolder(RecursionHolderInfo, BaseTypeInfo),
    Custom(BaseTypeInfo),
}

// Single source of truth for `enum TypeTag`, the class-name → tag mapping, and
// the tag → `Type` builder. Variants of `enum Type` above stay declared by
// hand because `macro_rules!` cannot generate enum variants from a flat list.
type_registry! {
    None                : "NoneType"               => Unit,
    Never               : "NeverType"              => Unit,
    Integer             : "IntegerType"            => Info(IntegerTypeInfo, extract_i64_number_type),
    Float               : "FloatType"              => Info(FloatTypeInfo, extract_f64_number_type),
    Decimal             : "DecimalType"            => Info(DecimalTypeInfo, extract_f64_number_type),
    String              : "StringType"             => Info(StringTypeInfo, StringTypeInfo::extract),
    Boolean             : "BooleanType"            => Unit,
    Uuid                : "UUIDType"               => Unit,
    Time                : "TimeType"               => Unit,
    DateTime            : "DateTimeType"           => Unit,
    Date                : "DateType"               => Unit,
    Bytes               : "BytesType"              => Unit,
    Any                 : "AnyType"                => Unit,
    Custom              : "CustomType"             => Unit,
    Enum                : "EnumType"               => Info(EnumTypeInfo, EnumTypeInfo::extract),
    Literal             : "LiteralType"            => Info(LiteralTypeInfo, LiteralTypeInfo::extract),
    RecursionHolder     : "RecursionHolder"        => Info(RecursionHolderInfo, RecursionHolderInfo::extract),
    Optional            : "OptionalType"           => CachedInfo(OptionalTypeInfo, OptionalTypeInfo::extract),
    Array               : "ArrayType"              => CachedInfo(ArrayTypeInfo, ArrayTypeInfo::extract),
    Dictionary          : "DictionaryType"         => CachedInfo(DictionaryTypeInfo, DictionaryTypeInfo::extract),
    Tuple               : "TupleType"              => CachedInfo(TupleTypeInfo, TupleTypeInfo::extract),
    Union               : "UnionType"              => CachedInfo(UnionTypeInfo, UnionTypeInfo::extract),
    DiscriminatedUnion  : "DiscriminatedUnionType" => CachedInfo(DiscriminatedUnionTypeInfo, DiscriminatedUnionTypeInfo::extract),
    Entity              : "EntityType"             => CachedInfo(EntityTypeInfo, EntityTypeInfo::extract),
    TypedDict           : "TypedDictType"          => CachedInfo(TypedDictTypeInfo, TypedDictTypeInfo::extract),
}

pub fn get_object_type(type_info: &Bound<'_, PyAny>) -> PyResult<Type> {
    let py = type_info.py();
    let registry = type_info_registry(py)?;
    let base_type = BaseTypeInfo::extract(type_info)?;
    let python_object_id = type_info.as_ptr() as *const _ as usize;
    let cls_id = type_info.get_type().as_ptr() as *const _ as usize;

    let Some(&tag) = registry.tags.get(&cls_id) else {
        return Err(PyRuntimeError::new_err(format!(
            "Unknown type-info object: {}",
            fmt_py(type_info)
        )));
    };

    build_type(tag, type_info, base_type, python_object_id)
}

struct TypeInfoRegistry {
    tags: IntMap<usize, TypeTag>,
    not_set: Py<PyAny>,
    // Keep classes alive so that their pointer keys in `tags` stay valid.
    _classes: Vec<Py<PyType>>,
}

static TYPE_INFO_REGISTRY: PyOnceLock<TypeInfoRegistry> = PyOnceLock::new();

fn type_info_registry(py: Python<'_>) -> PyResult<&TypeInfoRegistry> {
    TYPE_INFO_REGISTRY.get_or_try_init(py, || {
        let module = PyModule::import(py, "serpyco_rs._type_info")?;
        let mut tags = IntMap::default();
        let mut classes = Vec::with_capacity(TYPE_MAPPINGS.len());
        for (name, tag) in TYPE_MAPPINGS {
            let cls = module.getattr(*name)?.cast_into::<PyType>()?;
            tags.insert(cls.as_ptr() as *const _ as usize, *tag);
            classes.push(cls.unbind());
        }

        Ok(TypeInfoRegistry {
            tags,
            not_set: module.getattr("NOT_SET")?.unbind(),
            _classes: classes,
        })
    })
}

fn py_none_to_option(obj: Bound<'_, PyAny>) -> Option<Py<PyAny>> {
    if obj.is_none() {
        None
    } else {
        Some(obj.unbind())
    }
}

fn extract_default_value(default: Bound<'_, PyAny>) -> PyResult<Option<Py<PyAny>>> {
    let registry = type_info_registry(default.py())?;
    if default.is(registry.not_set.bind(default.py())) {
        Ok(None)
    } else {
        Ok(Some(default.unbind()))
    }
}

fn extract_fields(fields: Bound<'_, PyAny>) -> PyResult<Vec<EntityFieldInfo>> {
    let fields = fields.cast::<PyList>()?;
    fields
        .iter()
        .map(EntityFieldInfo::extract)
        .collect::<PyResult<Vec<_>>>()
}

fn extract_py_list(values: Bound<'_, PyAny>) -> PyResult<Vec<Py<PyAny>>> {
    let values = values.cast::<PyList>()?;
    Ok(values.iter().map(|x| x.unbind()).collect())
}

fn extract_used_keys(py: Python<'_>, used_keys: Bound<'_, PyAny>) -> PyResult<Py<PySet>> {
    if used_keys.is_none() {
        PySet::empty(py).map(|set| set.unbind())
    } else {
        Ok(used_keys.cast::<PySet>().map(|set| set.clone().unbind())?)
    }
}
