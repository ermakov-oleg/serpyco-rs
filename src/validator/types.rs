use pyo3::exceptions::PyRuntimeError;
use std::fmt;

use pyo3::prelude::*;
use pyo3::types::{PyNone, PyTuple, PyList};
use crate::python::get_value_attr;

use super::value::{Value};

macro_rules! py_eq {
    ($obj1:expr, $obj2:expr, $py:expr) => {
        $obj1.as_ref($py).eq($obj2.as_ref($py))?
    };
}

#[pyclass(frozen, module = "serde_json", subclass)]
#[derive(Debug, Clone)]
pub struct BaseType {
    #[pyo3(get)]
    pub custom_encoder: Option<Py<PyAny>>,
}

#[pymethods]
impl BaseType {
    #[new]
    fn new(custom_encoder: Option<&PyAny>) -> Self {
        BaseType {
            custom_encoder: custom_encoder.map(|x| x.into()),
        }
    }

    fn __repr__(&self) -> String {
        format!("<Type: custom_encoder={:?}>", self.custom_encoder)
    }

    fn __eq__(&self, other: &Self, py: Python<'_>) -> PyResult<bool> {
        match (&self.custom_encoder, &other.custom_encoder) {
            (Some(a), Some(b)) => Ok(py_eq!(a, b, py)),
            (None, None) => Ok(true),
            _ => Ok(false),
        }
    }
}

#[pyclass(frozen, module = "serde_json", subclass)]
#[derive(Debug, Clone)]
pub struct CustomEncoder {
    #[pyo3(get)]
    pub serialize: Option<Py<PyAny>>,
    #[pyo3(get)]
    pub deserialize: Option<Py<PyAny>>,
}

#[pymethods]
impl CustomEncoder {
    #[new]
    #[pyo3(signature = (serialize=None, deserialize=None))]
    fn new(serialize: Option<&PyAny>, deserialize: Option<&PyAny>) -> PyResult<Self> {
        Ok(CustomEncoder {
            serialize: serialize.map(|x| x.into()),
            deserialize: deserialize.map(|x| x.into()),
        })
    }

    fn __repr__(&self) -> String {
        format!(
            "<CustomEncoder: serialize={:?}, deserialize={:?}>",
            self.serialize, self.deserialize
        )
    }
}

#[pyclass(frozen, extends=BaseType, module = "serde_json")]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IntegerType {
    #[pyo3(get)]
    pub min: Option<i64>,
    #[pyo3(get)]
    pub max: Option<i64>,
}

#[pymethods]
impl IntegerType {
    #[new]
    fn new(min: Option<i64>, max: Option<i64>, custom_encoder: Option<&PyAny>) -> (Self, BaseType) {
        (IntegerType { min, max }, BaseType::new(custom_encoder))
    }

    fn __eq__(self_: PyRef<'_, Self>, other: PyRef<'_, Self>, py: Python<'_>) -> PyResult<bool> {
        let base = self_.as_ref();
        let base_other = other.as_ref();
        Ok(base.__eq__(base_other, py)? && self_.min == other.min && self_.max == other.max)
    }

    fn __repr__(&self) -> String {
        format!("<IntegerType: min={:?}, max={:?}>", self.min, self.max)
    }
}

#[pyclass(frozen, extends=BaseType, module = "serde_json")]
#[derive(Debug, Clone, PartialEq)]
pub struct FloatType {
    #[pyo3(get)]
    pub min: Option<f64>,
    #[pyo3(get)]
    pub max: Option<f64>,
}

#[pymethods]
impl FloatType {
    #[new]
    fn new(min: Option<f64>, max: Option<f64>, custom_encoder: Option<&PyAny>) -> (Self, BaseType) {
        (FloatType { min, max }, BaseType::new(custom_encoder))
    }

    fn __eq__(self_: PyRef<'_, Self>, other: PyRef<'_, Self>, py: Python<'_>) -> PyResult<bool> {
        let base = self_.as_ref();
        let base_other = other.as_ref();
        Ok(base.__eq__(base_other, py)? && self_.min == other.min && self_.max == other.max)
    }

    fn __repr__(&self) -> String {
        format!("<FloatType: min={:?}, max={:?}>", self.min, self.max)
    }
}

#[pyclass(frozen, extends=BaseType, module = "serde_json")]
#[derive(Debug, Clone, PartialEq)]
pub struct DecimalType {
    #[pyo3(get)]
    pub min: Option<f64>,
    #[pyo3(get)]
    pub max: Option<f64>,
}

#[pymethods]
impl DecimalType {
    #[new]
    fn new(min: Option<f64>, max: Option<f64>, custom_encoder: Option<&PyAny>) -> (Self, BaseType) {
        (DecimalType { min, max }, BaseType::new(custom_encoder))
    }

    fn __eq__(self_: PyRef<'_, Self>, other: PyRef<'_, Self>, py: Python<'_>) -> PyResult<bool> {
        let base = self_.as_ref();
        let base_other = other.as_ref();
        Ok(base.__eq__(base_other, py)? && self_.min == other.min && self_.max == other.max)
    }

    fn __repr__(&self) -> String {
        format!("<FloatType: min={:?}, max={:?}>", self.min, self.max)
    }
}

#[pyclass(frozen, extends=BaseType, module = "serde_json")]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StringType {
    #[pyo3(get)]
    pub min_length: Option<usize>,
    #[pyo3(get)]
    pub max_length: Option<usize>,
}

#[pymethods]
impl StringType {
    #[new]
    fn new(
        min_length: Option<usize>,
        max_length: Option<usize>,
        custom_encoder: Option<&PyAny>,
    ) -> (Self, BaseType) {
        (
            StringType {
                min_length,
                max_length,
            },
            BaseType::new(custom_encoder),
        )
    }

    fn __eq__(self_: PyRef<'_, Self>, other: PyRef<'_, Self>, py: Python<'_>) -> PyResult<bool> {
        let base = self_.as_ref();
        let base_other = other.as_ref();
        Ok(base.__eq__(base_other, py)?
            && self_.min_length == other.min_length
            && self_.max_length == other.max_length)
    }

    fn __repr__(&self) -> String {
        format!(
            "<StringType: min_length={:?}, max_length={:?}>",
            self.min_length, self.max_length
        )
    }
}

#[pyclass(frozen, extends=BaseType, module = "serde_json")]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BooleanType {}

#[pymethods]
impl BooleanType {
    #[new]
    fn new(custom_encoder: Option<&PyAny>) -> (Self, BaseType) {
        (BooleanType {}, BaseType::new(custom_encoder))
    }

    fn __eq__(self_: PyRef<'_, Self>, other: PyRef<'_, Self>, py: Python<'_>) -> PyResult<bool> {
        let base = self_.as_ref();
        let base_other = other.as_ref();
        base.__eq__(base_other, py)
    }

    fn __repr__(&self) -> String {
        "<BooleanType>".to_string()
    }
}

#[pyclass(frozen, extends=BaseType, module = "serde_json")]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UUIDType {}

#[pymethods]
impl UUIDType {
    #[new]
    fn new(custom_encoder: Option<&PyAny>) -> (Self, BaseType) {
        (UUIDType {}, BaseType::new(custom_encoder))
    }

    fn __eq__(self_: PyRef<'_, Self>, other: PyRef<'_, Self>, py: Python<'_>) -> PyResult<bool> {
        let base = self_.as_ref();
        let base_other = other.as_ref();
        base.__eq__(base_other, py)
    }

    fn __repr__(&self) -> String {
        "<UUIDType>".to_string()
    }
}

#[pyclass(frozen, extends=BaseType, module = "serde_json")]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TimeType {}

#[pymethods]
impl TimeType {
    #[new]
    fn new(custom_encoder: Option<&PyAny>) -> (Self, BaseType) {
        (TimeType {}, BaseType::new(custom_encoder))
    }

    fn __eq__(self_: PyRef<'_, Self>, other: PyRef<'_, Self>, py: Python<'_>) -> PyResult<bool> {
        let base = self_.as_ref();
        let base_other = other.as_ref();
        base.__eq__(base_other, py)
    }

    fn __repr__(&self) -> String {
        "<TimeType>".to_string()
    }
}

#[pyclass(frozen, extends=BaseType, module = "serde_json")]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DateTimeType {}

#[pymethods]
impl DateTimeType {
    #[new]
    fn new(custom_encoder: Option<&PyAny>) -> (Self, BaseType) {
        (DateTimeType {}, BaseType::new(custom_encoder))
    }

    fn __eq__(self_: PyRef<'_, Self>, other: PyRef<'_, Self>, py: Python<'_>) -> PyResult<bool> {
        let base = self_.as_ref();
        let base_other = other.as_ref();
        base.__eq__(base_other, py)
    }

    fn __repr__(&self) -> String {
        "<TimeType>".to_string()
    }
}

#[pyclass(frozen, extends=BaseType, module = "serde_json")]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DateType {}

#[pymethods]
impl DateType {
    #[new]
    fn new(custom_encoder: Option<&PyAny>) -> (Self, BaseType) {
        (DateType {}, BaseType::new(custom_encoder))
    }

    fn __eq__(self_: PyRef<'_, Self>, other: PyRef<'_, Self>, py: Python<'_>) -> PyResult<bool> {
        let base = self_.as_ref();
        let base_other = other.as_ref();
        base.__eq__(base_other, py)
    }

    fn __repr__(&self) -> String {
        "<TimeType>".to_string()
    }
}

#[derive(Debug, Clone)]
pub enum DefaultValueEnum {
    None,
    Value(Py<PyAny>),
}

#[pyclass(frozen, module = "serde_json", subclass)]
#[derive(Debug, Clone)]
pub struct DefaultValue(DefaultValueEnum);

#[pymethods]
impl DefaultValue {
    #[staticmethod]
    fn none() -> Self {
        Self(DefaultValueEnum::None)
    }
    #[staticmethod]
    fn some(value: &PyAny) -> Self {
        Self(DefaultValueEnum::Value(value.into()))
    }

    fn is_none(&self) -> bool {
        matches!(self.0, DefaultValueEnum::None)
    }

    fn __repr__(&self, py: Python<'_>) -> String {
        match &self.0 {
            DefaultValueEnum::None => "Rust None".to_string(),
            DefaultValueEnum::Value(value) => format!("{}", value.as_ref(py).repr().unwrap()),
        }
    }

    fn __eq__(&self, other: &Self) -> bool {
        self == other
    }

    fn __hash__(&self, py: Python<'_>) -> PyResult<isize> {
        match &self.0 {
            DefaultValueEnum::None => Ok(0),
            DefaultValueEnum::Value(value) => value.as_ref(py).hash(),
        }
    }
}

impl From<DefaultValue> for Option<Py<PyAny>> {
    fn from(val: DefaultValue) -> Self {
        match val.0 {
            DefaultValueEnum::None => None,
            DefaultValueEnum::Value(value) => Some(value),
        }
    }
}

impl PartialEq<Self> for DefaultValue {
    fn eq(&self, other: &Self) -> bool {
        match (&self.0, &other.0) {
            (DefaultValueEnum::None, DefaultValueEnum::None) => true,
            (DefaultValueEnum::Value(a), DefaultValueEnum::Value(b)) => {
                Python::with_gil(|py| a.as_ref(py).eq(b.as_ref(py)).unwrap_or(false))
            }
            _ => false,
        }
    }
}

#[pyclass(frozen, extends=BaseType, module = "serde_json")]
#[derive(Debug, Clone)]
pub struct EntityType {
    #[pyo3(get)]
    pub cls: Py<PyAny>,
    #[pyo3(get)]
    pub name: Py<PyAny>,
    #[pyo3(get)]
    pub fields: Vec<EntityField>,
    #[pyo3(get)]
    pub omit_none: bool,
    #[pyo3(get)]
    pub generics: Py<PyAny>,
    #[pyo3(get)]
    pub doc: Py<PyAny>,
}

#[pymethods]
impl EntityType {
    #[new]
    #[pyo3(signature = (cls, name, fields, omit_none=false, generics=None, doc=None, custom_encoder=None))]
    #[allow(clippy::too_many_arguments)]
    fn new(
        cls: &PyAny,
        name: &PyAny,
        fields: Vec<EntityField>,
        omit_none: bool,
        generics: Option<&PyAny>,
        doc: Option<&PyAny>,
        custom_encoder: Option<&PyAny>,
        py: Python<'_>,
    ) -> (Self, BaseType) {
        (
            EntityType {
                cls: cls.into(),
                name: name.into(),
                fields,
                omit_none,
                generics: generics.map_or(PyTuple::empty(py).into(), |x| x.into()),
                doc: doc.map_or(PyNone::get(py).into(), |x| x.into()),
            },
            BaseType::new(custom_encoder),
        )
    }

    fn __eq__(self_: PyRef<'_, Self>, other: PyRef<'_, Self>, py: Python<'_>) -> PyResult<bool> {
        let base = self_.as_ref();
        let base_other = other.as_ref();
        Ok(base.__eq__(base_other, py)?
            && py_eq!(self_.cls, other.cls, py)
            && py_eq!(self_.name, other.name, py)
            && self_.fields.len() == other.fields.len()
            && self_
                .fields
                .iter()
                .zip(other.fields.iter())
                .all(|(a, b)| a.__eq__(b, py).is_ok_and(|x| x))
            && self_.omit_none == other.omit_none
            && py_eq!(self_.generics, other.generics, py)
            && py_eq!(self_.doc, other.doc, py))
    }

    fn __repr__(&self) -> String {
        let fields = self
            .fields
            .iter()
            .map(|f| f.__repr__())
            .collect::<Vec<String>>()
            .join(", ");
        format!(
            "<EntityType: cls={:?}, name={:?}, fields=[{:?}], omit_none={:?}, generics={:?}, doc={:?}>",
            self.cls.to_string(),
            self.name.to_string(),
            fields,
            self.omit_none,
            self.generics.to_string(),
            self.doc.to_string()
        )
    }
}

#[pyclass(frozen, extends=BaseType, module = "serde_json")]
#[derive(Debug, Clone)]
pub struct TypedDictType {
    #[pyo3(get)]
    pub name: Py<PyAny>,
    #[pyo3(get)]
    pub fields: Vec<EntityField>,
    #[pyo3(get)]
    pub omit_none: bool,
    #[pyo3(get)]
    pub generics: Py<PyAny>,
    #[pyo3(get)]
    pub doc: Py<PyAny>,
}

#[pymethods]
impl TypedDictType {
    #[new]
    #[pyo3(signature = (name, fields, omit_none=false, generics=None, doc=None, custom_encoder=None))]
    fn new(
        name: &PyAny,
        fields: Vec<EntityField>,
        omit_none: bool,
        generics: Option<&PyAny>,
        doc: Option<&PyAny>,
        custom_encoder: Option<&PyAny>,
        py: Python<'_>,
    ) -> (Self, BaseType) {
        (
            TypedDictType {
                name: name.into(),
                fields,
                omit_none,
                generics: generics.map_or(PyTuple::empty(py).into(), |x| x.into()),
                doc: doc.map_or(PyNone::get(py).into(), |x| x.into()),
            },
            BaseType::new(custom_encoder),
        )
    }

    fn __eq__(self_: PyRef<'_, Self>, other: PyRef<'_, Self>, py: Python<'_>) -> PyResult<bool> {
        let base = self_.as_ref();
        let base_other = other.as_ref();
        Ok(base.__eq__(base_other, py)?
            && py_eq!(self_.name, other.name, py)
            && self_.fields.len() == other.fields.len()
            && self_
                .fields
                .iter()
                .zip(other.fields.iter())
                .all(|(a, b)| a.__eq__(b, py).is_ok_and(|x| x))
            && self_.omit_none == other.omit_none
            && py_eq!(self_.generics, other.generics, py)
            && py_eq!(self_.doc, other.doc, py))
    }

    fn __repr__(&self) -> String {
        let fields = self
            .fields
            .iter()
            .map(|f| f.__repr__())
            .collect::<Vec<String>>()
            .join(", ");
        format!(
            "<TypedDictType: name={:?}, fields=[{:?}], omit_none={:?}, generics={:?}, doc={:?}>",
            self.name.to_string(),
            fields,
            self.omit_none,
            self.generics.to_string(),
            self.doc.to_string()
        )
    }
}

#[pyclass(frozen, module = "serde_json")]
#[derive(Debug, Clone)]
pub struct EntityField {
    #[pyo3(get)]
    pub name: Py<PyAny>,
    #[pyo3(get)]
    pub dict_key: Py<PyAny>,
    #[pyo3(get)]
    pub field_type: Py<PyAny>,
    #[pyo3(get)]
    pub required: bool,
    #[pyo3(get)]
    pub is_discriminator_field: bool,
    #[pyo3(get)]
    pub default: DefaultValue,
    #[pyo3(get)]
    pub default_factory: DefaultValue,
    #[pyo3(get)]
    pub doc: Py<PyAny>,
}

#[pymethods]
impl EntityField {
    #[new]
    #[pyo3(signature = (name, dict_key, field_type, required=true, is_discriminator_field=false, default=DefaultValue(DefaultValueEnum::None), default_factory=DefaultValue(DefaultValueEnum::None), doc=None))]
    #[allow(clippy::too_many_arguments)]
    fn new(
        name: &PyAny,
        dict_key: &PyAny,
        field_type: &PyAny,
        required: bool,
        is_discriminator_field: bool,
        default: DefaultValue,
        default_factory: DefaultValue,
        doc: Option<&PyAny>,
        py: Python<'_>,
    ) -> Self {
        EntityField {
            name: name.into(),
            dict_key: dict_key.into(),
            field_type: field_type.into(),
            required,
            is_discriminator_field,
            doc: doc.map_or(PyNone::get(py).into(), |x| x.into()),
            default,
            default_factory,
        }
    }

    fn __eq__(&self, other: &Self, py: Python<'_>) -> PyResult<bool> {
        Ok(py_eq!(self.name, other.name, py)
            && py_eq!(self.dict_key, other.dict_key, py)
            && py_eq!(self.field_type, other.field_type, py)
            && self.required == other.required
            && self.is_discriminator_field == other.is_discriminator_field
            && self.default == other.default
            && self.default_factory == other.default_factory
            && py_eq!(self.doc, other.doc, py))
    }

    fn __repr__(&self) -> String {
        format!("<EntityField: name={:?}, dict_key={:?}, field_type={:?}, required={:?}, is_discriminator_field={:?}, default={:?}, default_factory={:?}, doc={:?}>", self.name.to_string(), self.dict_key.to_string(), self.field_type.to_string(), self.required, self.is_discriminator_field, self.default, self.default_factory, self.doc.to_string())
    }
}

#[pyclass(frozen, extends=BaseType, module = "serde_json")]
#[derive(Debug, Clone)]
pub struct ArrayType {
    #[pyo3(get)]
    pub item_type: Py<PyAny>,
}

#[pymethods]
impl ArrayType {
    #[new]
    #[pyo3(signature = (item_type, custom_encoder=None))]
    fn new(item_type: &PyAny, custom_encoder: Option<&PyAny>) -> (Self, BaseType) {
        (
            ArrayType {
                item_type: item_type.into(),
            },
            BaseType::new(custom_encoder),
        )
    }

    fn __eq__(self_: PyRef<'_, Self>, other: PyRef<'_, Self>, py: Python<'_>) -> PyResult<bool> {
        let base = self_.as_ref();
        let base_other = other.as_ref();
        Ok(base.__eq__(base_other, py)? && py_eq!(self_.item_type, other.item_type, py))
    }

    fn __repr__(&self) -> String {
        format!("<ArrayType: item_type={:?}>", self.item_type.to_string(),)
    }
}

#[pyclass(frozen, extends=BaseType, module = "serde_json")]
#[derive(Debug, Clone)]
pub struct EnumType {
    #[pyo3(get)]
    pub cls: Py<PyAny>,
    #[pyo3(get)]
    pub items: Py<PyList>,
    pub enum_items: Vec<(EnumItem, Py<PyAny>)>,
}

#[pymethods]
impl EnumType {
    #[new]
    #[pyo3(signature = (cls, items, custom_encoder=None))]
    fn new(cls: &PyAny, items: &PyList, custom_encoder: Option<&PyAny>) -> PyResult<(Self, BaseType)> {
        let mut enum_items = vec![];
        for py_item in items.iter() {
            let item = Value::new(get_value_attr(py_item.as_ptr())?);
            if let Some(str) = item.as_str() {
                enum_items.push((EnumItem::String(str.to_string()), py_item.into()));
            } else if let Some(int) = item.as_int() {
                enum_items.push((EnumItem::Int(int), py_item.into()));
            }
        }
        enum_items.sort_by(|a, b| a.0.cmp(&b.0));
        Ok((
            EnumType {
                cls: cls.into(),
                items: items.into(),
                enum_items,
            },
            BaseType::new(custom_encoder),
        ))
    }

    fn __eq__(self_: PyRef<'_, Self>, other: PyRef<'_, Self>, py: Python<'_>) -> PyResult<bool> {
        let base = self_.as_ref();
        let base_other = other.as_ref();
        Ok(base.__eq__(base_other, py)?
            && py_eq!(self_.cls, other.cls, py)
            && py_eq!(self_.items, other.items, py))
    }

    fn __repr__(&self) -> String {
        format!(
            "<EnumType: cls={:?}, items={:?}>",
            self.cls.to_string(),
            self.items.to_string(),
        )
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd)]
pub enum EnumItem {
    Int(i64),
    String(String),
}

pub struct EnumItems(Vec<EnumItem>);

impl fmt::Display for EnumItems {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut items = vec![];
        for item in &self.0 {
            match item {
                EnumItem::Int(int) => items.push(int.to_string()),
                EnumItem::String(str) => items.push(format!("\"{}\"", str)),
            }
        }
        write!(f, "[{}]", items.join(", "))
    }
}

impl<'a> From<&'a Vec<(EnumItem, Py<PyAny>)>> for EnumItems {
    fn from(items: &'a Vec<(EnumItem, Py<PyAny>)>) -> Self {
        EnumItems(items.iter().map(|(item, _)| item.clone()).collect::<Vec<_>>())
    }
}

#[pyclass(frozen, extends=BaseType, module = "serde_json")]
#[derive(Debug, Clone)]
pub struct OptionalType {
    #[pyo3(get)]
    pub inner: Py<PyAny>,
}

#[pymethods]
impl OptionalType {
    #[new]
    #[pyo3(signature = (inner, custom_encoder=None))]
    fn new(inner: &PyAny, custom_encoder: Option<&PyAny>) -> (Self, BaseType) {
        (
            OptionalType {
                inner: inner.into(),
            },
            BaseType::new(custom_encoder),
        )
    }

    fn __eq__(self_: PyRef<'_, Self>, other: PyRef<'_, Self>, py: Python<'_>) -> PyResult<bool> {
        let base = self_.as_ref();
        let base_other = other.as_ref();
        Ok(base.__eq__(base_other, py)? && py_eq!(self_.inner, other.inner, py))
    }

    fn __repr__(&self) -> String {
        format!("<OptionalType: inner={:?}>", self.inner.to_string(),)
    }
}

#[pyclass(frozen, extends=BaseType, module = "serde_json")]
#[derive(Debug, Clone)]
pub struct DictionaryType {
    #[pyo3(get)]
    pub key_type: Py<PyAny>,
    #[pyo3(get)]
    pub value_type: Py<PyAny>,
    #[pyo3(get)]
    pub omit_none: bool,
}

#[pymethods]
impl DictionaryType {
    #[new]
    #[pyo3(signature = (key_type, value_type, omit_none=false, custom_encoder=None))]
    fn new(
        key_type: &PyAny,
        value_type: &PyAny,
        omit_none: bool,
        custom_encoder: Option<&PyAny>,
    ) -> (Self, BaseType) {
        (
            DictionaryType {
                key_type: key_type.into(),
                value_type: value_type.into(),
                omit_none,
            },
            BaseType::new(custom_encoder),
        )
    }

    fn __eq__(self_: PyRef<'_, Self>, other: PyRef<'_, Self>, py: Python<'_>) -> PyResult<bool> {
        let base = self_.as_ref();
        let base_other = other.as_ref();
        Ok(base.__eq__(base_other, py)?
            && py_eq!(self_.key_type, other.key_type, py)
            && py_eq!(self_.value_type, other.value_type, py)
            && self_.omit_none == other.omit_none)
    }

    fn __repr__(&self) -> String {
        format!(
            "<DictionaryType: key_type={:?}, value_type={:?}, omit_none={:?}>",
            self.key_type.to_string(),
            self.value_type.to_string(),
            self.omit_none,
        )
    }
}

#[pyclass(frozen, extends=BaseType, module = "serde_json")]
#[derive(Debug, Clone)]
pub struct TupleType {
    #[pyo3(get)]
    pub item_types: Vec<Py<PyAny>>,
}

#[pymethods]
impl TupleType {
    #[new]
    #[pyo3(signature = (item_types, custom_encoder=None))]
    fn new(item_types: Vec<&PyAny>, custom_encoder: Option<&PyAny>) -> (Self, BaseType) {
        (
            TupleType {
                item_types: item_types.into_iter().map(|x| x.into()).collect(),
            },
            BaseType::new(custom_encoder),
        )
    }

    fn __eq__(self_: PyRef<'_, Self>, other: PyRef<'_, Self>, py: Python<'_>) -> PyResult<bool> {
        let base = self_.as_ref();
        let base_other = other.as_ref();
        Ok(base.__eq__(base_other, py)?
            && self_.item_types.len() == other.item_types.len()
            && self_
                .item_types
                .iter()
                .zip(other.item_types.iter())
                .all(|(a, b)| a.as_ref(py).eq(b.as_ref(py)).unwrap_or(false)))
    }

    fn __repr__(&self) -> String {
        let item_types = self
            .item_types
            .iter()
            .map(|f| f.to_string())
            .collect::<Vec<String>>()
            .join(", ");
        format!("<TupleType: item_types={:?}>", item_types)
    }
}

#[pyclass(frozen, extends=BaseType, module = "serde_json")]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BytesType {}

#[pymethods]
impl BytesType {
    #[new]
    fn new(custom_encoder: Option<&PyAny>) -> (Self, BaseType) {
        (BytesType {}, BaseType::new(custom_encoder))
    }

    fn __eq__(self_: PyRef<'_, Self>, other: PyRef<'_, Self>, py: Python<'_>) -> PyResult<bool> {
        let base = self_.as_ref();
        let base_other = other.as_ref();
        base.__eq__(base_other, py)
    }

    fn __repr__(&self) -> String {
        "<BytesType>".to_string()
    }
}

#[pyclass(frozen, extends=BaseType, module = "serde_json")]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AnyType {}

#[pymethods]
impl AnyType {
    #[new]
    fn new(custom_encoder: Option<&PyAny>) -> (Self, BaseType) {
        (AnyType {}, BaseType::new(custom_encoder))
    }

    fn __eq__(self_: PyRef<'_, Self>, other: PyRef<'_, Self>, py: Python<'_>) -> PyResult<bool> {
        let base = self_.as_ref();
        let base_other = other.as_ref();
        base.__eq__(base_other, py)
    }

    fn __repr__(&self) -> String {
        "<AnyType>".to_string()
    }
}

#[pyclass(frozen, extends=BaseType, module = "serde_json")]
#[derive(Debug, Clone)]
pub struct UnionType {
    #[pyo3(get)]
    pub item_types: Py<PyAny>,
    #[pyo3(get)]
    pub dump_discriminator: Py<PyAny>,
    #[pyo3(get)]
    pub load_discriminator: Py<PyAny>,
}

#[pymethods]
impl UnionType {
    #[new]
    #[pyo3(signature = (item_types, dump_discriminator, load_discriminator, custom_encoder=None))]
    fn new(
        item_types: &PyAny,
        dump_discriminator: &PyAny,
        load_discriminator: &PyAny,
        custom_encoder: Option<&PyAny>,
    ) -> (Self, BaseType) {
        (
            UnionType {
                item_types: item_types.into(),
                dump_discriminator: dump_discriminator.into(),
                load_discriminator: load_discriminator.into(),
            },
            BaseType::new(custom_encoder),
        )
    }

    fn __eq__(self_: PyRef<'_, Self>, other: PyRef<'_, Self>, py: Python<'_>) -> PyResult<bool> {
        let base = self_.as_ref();
        let base_other = other.as_ref();
        Ok(base.__eq__(base_other, py)?
            && py_eq!(self_.item_types, other.item_types, py)
            && py_eq!(self_.dump_discriminator, other.dump_discriminator, py)
            && py_eq!(self_.load_discriminator, other.load_discriminator, py))
    }

    fn __repr__(&self) -> String {
        format!(
            "<UnionType: item_types={:?}, dump_discriminator={:?}, load_discriminator={:?}>",
            self.item_types.to_string(),
            self.dump_discriminator.to_string(),
            self.load_discriminator.to_string(),
        )
    }
}

#[pyclass(frozen, extends=BaseType, module = "serde_json")]
#[derive(Debug, Clone)]
pub struct LiteralType {
    #[pyo3(get)]
    pub args: Py<PyList>,
    pub enum_items: Vec<(EnumItem, Py<PyAny>)>,
}

#[pymethods]
impl LiteralType {
    #[new]
    #[pyo3(signature = (args, custom_encoder=None))]
    fn new(args: &PyList, custom_encoder: Option<&PyAny>) -> (Self, BaseType) {
        let mut enum_items = vec![];
        for py_value in args.iter() {
            let item = Value::new(py_value.as_ptr());
            if let Some(str) = item.as_str() {
                enum_items.push((EnumItem::String(str.to_string()), py_value.into()));
            } else if let Some(int) = item.as_int() {
                enum_items.push((EnumItem::Int(int), py_value.into()));
            }
        }
        enum_items.sort_by(|a, b| a.0.cmp(&b.0));
        (
            LiteralType {
                args: args.into(),
                enum_items,
            },
            BaseType::new(custom_encoder),
        )
    }

    fn __eq__(self_: PyRef<'_, Self>, other: PyRef<'_, Self>, py: Python<'_>) -> PyResult<bool> {
        let base = self_.as_ref();
        let base_other = other.as_ref();
        Ok(base.__eq__(base_other, py)? && py_eq!(self_.args, other.args, py))
    }

    fn __repr__(&self) -> String {
        format!("<LiteralType: items={:?}>", self.args.to_string(),)
    }
}

#[pyclass(frozen, extends=BaseType, module = "serde_json")]
#[derive(Debug, Clone)]
pub struct RecursionHolder {
    #[pyo3(get)]
    pub name: Py<PyAny>,
    pub state_key: Py<PyAny>,
    pub meta: Py<PyAny>,
}

#[pymethods]
impl RecursionHolder {
    #[new]
    #[pyo3(signature = (name, state_key, meta, custom_encoder=None))]
    fn new(
        name: &PyAny,
        state_key: &PyAny,
        meta: &PyAny,
        custom_encoder: Option<&PyAny>,
    ) -> (Self, BaseType) {
        (
            RecursionHolder {
                name: name.into(),
                state_key: state_key.into(),
                meta: meta.into(),
            },
            BaseType::new(custom_encoder),
        )
    }

    pub fn get_type<'a>(&'a self, py: Python<'a>) -> PyResult<&'a PyAny> {
        match self.meta.as_ref(py).get_item(&self.state_key) {
            Ok(type_) => Ok(type_),
            Err(e) => Err(PyErr::new::<PyRuntimeError, _>(format!(
                "Recursive type not resolved: {}",
                e
            ))),
        }
    }

    fn __eq__(self_: PyRef<'_, Self>, other: PyRef<'_, Self>, py: Python<'_>) -> PyResult<bool> {
        let base = self_.as_ref();
        let base_other = other.as_ref();
        Ok(base.__eq__(base_other, py)?
            && py_eq!(self_.name, other.name, py)
            && py_eq!(self_.state_key, other.state_key, py)
            && py_eq!(self_.meta, other.meta, py))
    }

    fn __repr__(&self) -> String {
        format!(
            "<RecursionHolder: name={:?}, state_key={:?}>",
            self.name.to_string(),
            self.state_key.to_string(),
        )
    }
}
