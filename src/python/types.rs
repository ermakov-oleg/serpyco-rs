use std::fmt;

use nohash_hasher::IntMap;
use pyo3::exceptions::PyRuntimeError;
use pyo3::prelude::{PyAnyMethods, PyModule};
use pyo3::types::{PyDict, PyInt, PyList, PyListMethods, PySet};
use pyo3::{intern, Bound, Py, PyAny, PyResult, Python};

use crate::python::fmt_py;

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
    pub state_key: Py<PyAny>,
    pub meta: Py<PyAny>,
}

impl RecursionHolderInfo {
    fn extract(type_info: &Bound<'_, PyAny>) -> PyResult<Self> {
        Ok(Self {
            state_key: type_info.getattr("state_key")?.unbind(),
            meta: type_info.getattr("meta")?.unbind(),
        })
    }

    pub fn get_inner_type<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        match self.meta.bind(py).get_item(&self.state_key) {
            Ok(type_) => Ok(type_),
            Err(e) => Err(PyRuntimeError::new_err(format!(
                "Recursive type not resolved: {e}"
            ))),
        }
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

pub fn get_object_type(type_info: &Bound<'_, PyAny>) -> PyResult<Type> {
    let base_type = BaseTypeInfo::extract(type_info)?;
    let python_object_id = type_info.as_ptr() as *const _ as usize;

    if is_type_info(type_info, "NoneType")? {
        return Ok(Type::None(base_type));
    }
    if is_type_info(type_info, "NeverType")? {
        return Ok(Type::Never(base_type));
    }
    if is_type_info(type_info, "IntegerType")? {
        return Ok(Type::Integer(
            extract_i64_number_type(type_info)?,
            base_type,
        ));
    }
    if is_type_info(type_info, "FloatType")? {
        return Ok(Type::Float(extract_f64_number_type(type_info)?, base_type));
    }
    if is_type_info(type_info, "DecimalType")? {
        return Ok(Type::Decimal(
            extract_f64_number_type(type_info)?,
            base_type,
        ));
    }
    if is_type_info(type_info, "StringType")? {
        return Ok(Type::String(StringTypeInfo::extract(type_info)?, base_type));
    }
    if is_type_info(type_info, "BooleanType")? {
        return Ok(Type::Boolean(base_type));
    }
    if is_type_info(type_info, "UUIDType")? {
        return Ok(Type::Uuid(base_type));
    }
    if is_type_info(type_info, "TimeType")? {
        return Ok(Type::Time(base_type));
    }
    if is_type_info(type_info, "DateTimeType")? {
        return Ok(Type::DateTime(base_type));
    }
    if is_type_info(type_info, "DateType")? {
        return Ok(Type::Date(base_type));
    }
    if is_type_info(type_info, "EnumType")? {
        return Ok(Type::Enum(EnumTypeInfo::extract(type_info)?, base_type));
    }
    if is_type_info(type_info, "LiteralType")? {
        return Ok(Type::Literal(
            LiteralTypeInfo::extract(type_info)?,
            base_type,
        ));
    }
    if is_type_info(type_info, "BytesType")? {
        return Ok(Type::Bytes(base_type));
    }
    if is_type_info(type_info, "RecursionHolder")? {
        return Ok(Type::RecursionHolder(
            RecursionHolderInfo::extract(type_info)?,
            base_type,
        ));
    }
    if is_type_info(type_info, "CustomType")? {
        return Ok(Type::Custom(base_type));
    }
    if is_type_info(type_info, "AnyType")? {
        return Ok(Type::Any(base_type));
    }
    if is_type_info(type_info, "OptionalType")? {
        return Ok(Type::Optional(
            OptionalTypeInfo::extract(type_info)?,
            base_type,
            python_object_id,
        ));
    }
    if is_type_info(type_info, "ArrayType")? {
        return Ok(Type::Array(
            ArrayTypeInfo::extract(type_info)?,
            base_type,
            python_object_id,
        ));
    }
    if is_type_info(type_info, "DictionaryType")? {
        return Ok(Type::Dictionary(
            DictionaryTypeInfo::extract(type_info)?,
            base_type,
            python_object_id,
        ));
    }
    if is_type_info(type_info, "TupleType")? {
        return Ok(Type::Tuple(
            TupleTypeInfo::extract(type_info)?,
            base_type,
            python_object_id,
        ));
    }
    if is_type_info(type_info, "UnionType")? {
        return Ok(Type::Union(
            UnionTypeInfo::extract(type_info)?,
            base_type,
            python_object_id,
        ));
    }
    if is_type_info(type_info, "DiscriminatedUnionType")? {
        return Ok(Type::DiscriminatedUnion(
            DiscriminatedUnionTypeInfo::extract(type_info)?,
            base_type,
            python_object_id,
        ));
    }
    if is_type_info(type_info, "EntityType")? {
        return Ok(Type::Entity(
            EntityTypeInfo::extract(type_info)?,
            base_type,
            python_object_id,
        ));
    }
    if is_type_info(type_info, "TypedDictType")? {
        return Ok(Type::TypedDict(
            TypedDictTypeInfo::extract(type_info)?,
            base_type,
            python_object_id,
        ));
    }

    unreachable!("Unknown type: {:?}", type_info)
}

fn is_type_info(type_info: &Bound<'_, PyAny>, type_name: &str) -> PyResult<bool> {
    let type_info_module = PyModule::import(type_info.py(), "serpyco_rs._type_info")?;
    let cls = type_info_module.getattr(type_name)?;
    type_info.is_instance(&cls)
}

fn py_none_to_option(obj: Bound<'_, PyAny>) -> Option<Py<PyAny>> {
    if obj.is_none() {
        None
    } else {
        Some(obj.unbind())
    }
}

fn extract_default_value(default: Bound<'_, PyAny>) -> PyResult<Option<Py<PyAny>>> {
    let type_info_module = PyModule::import(default.py(), "serpyco_rs._type_info")?;
    let not_set = type_info_module.getattr("NOT_SET")?;

    if default.is(&not_set) {
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
