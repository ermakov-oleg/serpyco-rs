use pyo3::exceptions::PyRuntimeError;
use pyo3::{intern, BoundObject};

use crate::python::fmt_py;
use pyo3::prelude::*;
use pyo3::types::{PyDict, PyInt, PyList, PyNone};

macro_rules! py_eq {
    ($obj1:expr, $obj2:expr, $py:expr) => {
        $obj1.bind($py).eq($obj2.bind($py))?
    };
}

#[pyclass(frozen, module = "serpyco_rs", subclass)]
#[derive(Debug, Clone)]
pub struct BaseType {
    #[pyo3(get)]
    pub custom_encoder: Option<Py<PyAny>>,
}

#[pymethods]
impl BaseType {
    #[new]
    #[pyo3(signature = (custom_encoder=None))]
    fn new(custom_encoder: Option<&Bound<'_, PyAny>>) -> Self {
        BaseType {
            custom_encoder: custom_encoder.map(|x| x.clone().unbind()),
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

#[pyclass(frozen, module = "serpyco_rs", subclass)]
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
    fn new(
        serialize: Option<&Bound<'_, PyAny>>,
        deserialize: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<Self> {
        Ok(CustomEncoder {
            serialize: serialize.map(|x| x.clone().unbind()),
            deserialize: deserialize.map(|x| x.clone().unbind()),
        })
    }

    fn __repr__(&self) -> String {
        format!(
            "<CustomEncoder: serialize={:?}, deserialize={:?}>",
            self.serialize, self.deserialize
        )
    }
}

#[pyclass(frozen, extends=BaseType, module="serpyco_rs")]
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
    #[pyo3(signature = (min=None, max=None, custom_encoder=None))]
    fn new(
        min: Option<i64>,
        max: Option<i64>,
        custom_encoder: Option<&Bound<'_, PyAny>>,
    ) -> (Self, BaseType) {
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

#[pyclass(frozen, extends=BaseType, module="serpyco_rs")]
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
    #[pyo3(signature = (min=None, max=None, custom_encoder=None))]
    fn new(
        min: Option<f64>,
        max: Option<f64>,
        custom_encoder: Option<&Bound<'_, PyAny>>,
    ) -> (Self, BaseType) {
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

#[pyclass(frozen, extends=BaseType, module="serpyco_rs")]
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
    #[pyo3(signature = (min=None, max=None, custom_encoder=None))]
    fn new(
        min: Option<f64>,
        max: Option<f64>,
        custom_encoder: Option<&Bound<'_, PyAny>>,
    ) -> (Self, BaseType) {
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

#[pyclass(frozen, extends=BaseType, module="serpyco_rs")]
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
    #[pyo3(signature = (min_length=None, max_length=None, custom_encoder=None))]
    fn new(
        min_length: Option<usize>,
        max_length: Option<usize>,
        custom_encoder: Option<&Bound<'_, PyAny>>,
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

#[pyclass(frozen, extends=BaseType, module="serpyco_rs")]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BooleanType {}

#[pymethods]
impl BooleanType {
    #[new]
    #[pyo3(signature = (custom_encoder=None))]
    fn new(custom_encoder: Option<&Bound<'_, PyAny>>) -> (Self, BaseType) {
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

#[pyclass(frozen, extends=BaseType, module="serpyco_rs")]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UUIDType {}

#[pymethods]
impl UUIDType {
    #[new]
    #[pyo3(signature = (custom_encoder=None))]
    fn new(custom_encoder: Option<&Bound<'_, PyAny>>) -> (Self, BaseType) {
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

#[pyclass(frozen, extends=BaseType, module="serpyco_rs")]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TimeType {}

#[pymethods]
impl TimeType {
    #[new]
    #[pyo3(signature = (custom_encoder=None))]
    fn new(custom_encoder: Option<&Bound<'_, PyAny>>) -> (Self, BaseType) {
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

#[pyclass(frozen, extends=BaseType, module="serpyco_rs")]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DateTimeType {}

#[pymethods]
impl DateTimeType {
    #[new]
    #[pyo3(signature = (custom_encoder=None))]
    fn new(custom_encoder: Option<&Bound<'_, PyAny>>) -> (Self, BaseType) {
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

#[pyclass(frozen, extends=BaseType, module="serpyco_rs")]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DateType {}

#[pymethods]
impl DateType {
    #[new]
    #[pyo3(signature = (custom_encoder=None))]
    fn new(custom_encoder: Option<&Bound<'_, PyAny>>) -> (Self, BaseType) {
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

#[pyclass(frozen, module = "serpyco_rs", subclass)]
#[derive(Debug, Clone)]
pub struct DefaultValue(DefaultValueEnum);

#[pymethods]
impl DefaultValue {
    #[staticmethod]
    fn none() -> Self {
        Self(DefaultValueEnum::None)
    }
    #[staticmethod]
    fn some(value: &Bound<'_, PyAny>) -> Self {
        Self(DefaultValueEnum::Value(value.clone().unbind()))
    }

    fn is_none(&self) -> bool {
        matches!(self.0, DefaultValueEnum::None)
    }

    fn __repr__(&self, py: Python<'_>) -> String {
        match &self.0 {
            DefaultValueEnum::None => "Rust None".to_string(),
            DefaultValueEnum::Value(value) => format!("{}", value.bind(py).repr().unwrap()),
        }
    }

    fn __eq__(&self, other: &Self) -> bool {
        self == other
    }

    fn __hash__(&self, py: Python<'_>) -> PyResult<isize> {
        match &self.0 {
            DefaultValueEnum::None => Ok(0),
            DefaultValueEnum::Value(value) => value.bind(py).hash(),
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
                Python::with_gil(|py| a.bind(py).eq(b.bind(py)).unwrap_or(false))
            }
            _ => false,
        }
    }
}

#[pyclass(frozen, extends=BaseType, module="serpyco_rs")]
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
    pub is_frozen: bool,
    #[pyo3(get)]
    pub doc: Py<PyAny>,
}

#[pymethods]
impl EntityType {
    #[new]
    #[pyo3(signature = (cls, name, fields, omit_none=false, is_frozen=false, doc=None, custom_encoder=None))]
    #[allow(clippy::too_many_arguments)]
    fn new(
        cls: &Bound<'_, PyAny>,
        name: &Bound<'_, PyAny>,
        fields: Vec<EntityField>,
        omit_none: bool,
        is_frozen: bool,
        doc: Option<&Bound<'_, PyAny>>,
        custom_encoder: Option<&Bound<'_, PyAny>>,
        py: Python<'_>,
    ) -> (Self, BaseType) {
        (
            EntityType {
                cls: cls.clone().unbind(),
                name: name.clone().unbind(),
                fields,
                omit_none,
                is_frozen,
                doc: doc.map_or(PyNone::get(py).into_any().unbind(), |x| x.clone().unbind()),
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
            "<EntityType: cls={:?}, name={:?}, fields=[{:?}], omit_none={:?}, doc={:?}>",
            self.cls.to_string(),
            self.name.to_string(),
            fields,
            self.omit_none,
            self.doc.to_string()
        )
    }
}

#[pyclass(frozen, extends=BaseType, module="serpyco_rs")]
#[derive(Debug, Clone)]
pub struct TypedDictType {
    #[pyo3(get)]
    pub name: Py<PyAny>,
    #[pyo3(get)]
    pub fields: Vec<EntityField>,
    #[pyo3(get)]
    pub omit_none: bool,
    #[pyo3(get)]
    pub doc: Py<PyAny>,
}

#[pymethods]
impl TypedDictType {
    #[new]
    #[pyo3(signature = (name, fields, omit_none=false, doc=None, custom_encoder=None))]
    fn new(
        name: &Bound<'_, PyAny>,
        fields: Vec<EntityField>,
        omit_none: bool,
        doc: Option<&Bound<'_, PyAny>>,
        custom_encoder: Option<&Bound<'_, PyAny>>,
        py: Python<'_>,
    ) -> (Self, BaseType) {
        (
            TypedDictType {
                name: name.clone().unbind(),
                fields,
                omit_none,
                doc: doc.map_or(PyNone::get(py).into_any().unbind(), |x| x.clone().unbind()),
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
            "<TypedDictType: name={:?}, fields=[{:?}], omit_none={:?}, doc={:?}>",
            self.name.to_string(),
            fields,
            self.omit_none,
            self.doc.to_string()
        )
    }
}

#[pyclass(frozen, module = "serpyco_rs")]
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
        name: &Bound<'_, PyAny>,
        dict_key: &Bound<'_, PyAny>,
        field_type: &Bound<'_, PyAny>,
        required: bool,
        is_discriminator_field: bool,
        default: DefaultValue,
        default_factory: DefaultValue,
        doc: Option<&Bound<'_, PyAny>>,
        py: Python<'_>,
    ) -> Self {
        EntityField {
            name: name.clone().unbind(),
            dict_key: dict_key.clone().unbind(),
            field_type: field_type.clone().clone().unbind(),
            required,
            is_discriminator_field,
            doc: doc.map_or(PyNone::get(py).into_any().unbind(), |x| x.clone().unbind()),
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

#[pyclass(frozen, extends=BaseType, module="serpyco_rs")]
#[derive(Debug, Clone)]
pub struct ArrayType {
    #[pyo3(get)]
    pub item_type: Py<PyAny>,
    #[pyo3(get)]
    pub min_length: Option<usize>,
    #[pyo3(get)]
    pub max_length: Option<usize>,
}

#[pymethods]
impl ArrayType {
    #[new]
    #[pyo3(signature = (item_type, min_length=None, max_length=None, custom_encoder=None))]
    fn new(
        item_type: &Bound<'_, PyAny>,
        min_length: Option<usize>,
        max_length: Option<usize>,
        custom_encoder: Option<&Bound<'_, PyAny>>,
    ) -> (Self, BaseType) {
        (
            ArrayType {
                item_type: item_type.clone().unbind(),
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
            && py_eq!(self_.item_type, other.item_type, py)
            && self_.min_length == other.min_length
            && self_.max_length == other.max_length)
    }

    fn __repr__(&self) -> String {
        format!(
            "<ArrayType: item_type={:?}, min_length={:?}, max_length={:?}>",
            self.item_type.to_string(),
            self.min_length,
            self.max_length
        )
    }
}

#[pyclass(frozen, extends=BaseType, module="serpyco_rs")]
#[derive(Debug, Clone)]
pub struct EnumType {
    #[pyo3(get)]
    pub cls: Py<PyAny>,
    #[pyo3(get)]
    pub items: Py<PyList>,
    // Map from expected values hash to the actual value
    pub load_map: Py<PyDict>,
    // Map from value hash to the expected value
    pub dump_map: Py<PyDict>,
    pub items_repr: String,
}

#[pymethods]
impl EnumType {
    #[new]
    #[pyo3(signature = (cls, items, custom_encoder=None))]
    fn new(
        cls: &Bound<'_, PyAny>,
        items: &Bound<'_, PyList>,
        custom_encoder: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<(Self, BaseType)> {
        let load_map = PyDict::new(cls.py());
        let dump_map = PyDict::new(cls.py());
        let mut items_repr = Vec::with_capacity(items.len());

        for py_value in items.iter() {
            // Get enum value
            let value = py_value.getattr(intern!(py_value.py(), "value")).unwrap();

            let py_value_id = py_value.as_ptr() as *const _ as usize;
            dump_map.set_item(py_value_id, value.clone())?;
            load_map.set_item(&value, &py_value)?;
            items_repr.push(fmt_py(&value));

            // For support fast load with try_cast_from_string option enabled
            // we need to add additional mapping for string values
            if let Ok(value) = value.downcast::<PyInt>() {
                let str_value = value.str().unwrap();
                load_map.set_item((&str_value, false), &py_value)?;
            }
        }

        Ok((
            EnumType {
                cls: cls.clone().unbind(),
                items: items.clone().unbind(),
                items_repr: format!("[{}]", items_repr.join(", ")),
                load_map: load_map.unbind(),
                dump_map: dump_map.unbind(),
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

#[pyclass(frozen, extends=BaseType, module="serpyco_rs")]
#[derive(Debug, Clone)]
pub struct OptionalType {
    #[pyo3(get)]
    pub inner: Py<PyAny>,
}

#[pymethods]
impl OptionalType {
    #[new]
    #[pyo3(signature = (inner, custom_encoder=None))]
    fn new(
        inner: &Bound<'_, PyAny>,
        custom_encoder: Option<&Bound<'_, PyAny>>,
    ) -> (Self, BaseType) {
        (
            OptionalType {
                inner: inner.clone().unbind(),
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

#[pyclass(frozen, extends=BaseType, module="serpyco_rs")]
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
        key_type: &Bound<'_, PyAny>,
        value_type: &Bound<'_, PyAny>,
        omit_none: bool,
        custom_encoder: Option<&Bound<'_, PyAny>>,
    ) -> (Self, BaseType) {
        (
            DictionaryType {
                key_type: key_type.clone().unbind(),
                value_type: value_type.clone().unbind(),
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

#[pyclass(frozen, extends=BaseType, module="serpyco_rs")]
#[derive(Debug, Clone)]
pub struct TupleType {
    #[pyo3(get)]
    pub item_types: Vec<Py<PyAny>>,
}

#[pymethods]
impl TupleType {
    #[new]
    #[pyo3(signature = (item_types, custom_encoder=None))]
    fn new(
        item_types: Vec<Bound<'_, PyAny>>,
        custom_encoder: Option<&Bound<'_, PyAny>>,
    ) -> (Self, BaseType) {
        (
            TupleType {
                item_types: item_types.into_iter().map(|x| x.unbind()).collect(),
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
                .all(|(a, b)| a.bind(py).eq(b.bind(py)).unwrap_or(false)))
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

#[pyclass(frozen, extends=BaseType, module="serpyco_rs")]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BytesType {}

#[pymethods]
impl BytesType {
    #[new]
    #[pyo3(signature = (custom_encoder=None))]
    fn new(custom_encoder: Option<&Bound<'_, PyAny>>) -> (Self, BaseType) {
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

#[pyclass(frozen, extends=BaseType, module="serpyco_rs")]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AnyType {}

#[pymethods]
impl AnyType {
    #[new]
    #[pyo3(signature = (custom_encoder=None))]
    fn new(custom_encoder: Option<&Bound<'_, PyAny>>) -> (Self, BaseType) {
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

#[pyclass(frozen, extends=BaseType, module="serpyco_rs")]
#[derive(Debug, Clone)]
pub struct DiscriminatedUnionType {
    #[pyo3(get)]
    pub item_types: Py<PyAny>,
    #[pyo3(get)]
    pub dump_discriminator: Py<PyAny>,
    #[pyo3(get)]
    pub load_discriminator: Py<PyAny>,
}

#[pymethods]
impl DiscriminatedUnionType {
    #[new]
    #[pyo3(signature = (item_types, dump_discriminator, load_discriminator, custom_encoder=None))]
    fn new(
        item_types: &Bound<'_, PyAny>,
        dump_discriminator: &Bound<'_, PyAny>,
        load_discriminator: &Bound<'_, PyAny>,
        custom_encoder: Option<&Bound<'_, PyAny>>,
    ) -> (Self, BaseType) {
        (
            DiscriminatedUnionType {
                item_types: item_types.clone().unbind(),
                dump_discriminator: dump_discriminator.clone().unbind(),
                load_discriminator: load_discriminator.clone().unbind(),
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
            "<DiscriminatedUnionType: item_types={:?}, dump_discriminator={:?}, load_discriminator={:?}>",
            self.item_types.to_string(),
            self.dump_discriminator.to_string(),
            self.load_discriminator.to_string(),
        )
    }
}

#[pyclass(frozen, extends=BaseType, module="serpyco_rs")]
#[derive(Debug, Clone)]
pub struct UnionType {
    #[pyo3(get)]
    pub item_types: Py<PyAny>,
    pub union_repr: String,
}

#[pymethods]
impl UnionType {
    #[new]
    #[pyo3(signature = (item_types, union_repr, custom_encoder=None))]
    fn new(
        item_types: &Bound<'_, PyAny>,
        union_repr: String,
        custom_encoder: Option<&Bound<'_, PyAny>>,
    ) -> (Self, BaseType) {
        (
            UnionType {
                item_types: item_types.clone().unbind(),
                union_repr,
            },
            BaseType::new(custom_encoder),
        )
    }

    fn __eq__(self_: PyRef<'_, Self>, other: PyRef<'_, Self>, py: Python<'_>) -> PyResult<bool> {
        let base = self_.as_ref();
        let base_other = other.as_ref();
        Ok(base.__eq__(base_other, py)?
            && py_eq!(self_.item_types, other.item_types, py)
            && self_.union_repr == other.union_repr)
    }

    fn __repr__(&self) -> String {
        format!("<UnionType: item_types={:?}>", self.item_types.to_string(),)
    }
}

#[pyclass(frozen, extends=BaseType, module="serpyco_rs")]
#[derive(Debug, Clone)]
pub struct LiteralType {
    #[pyo3(get)]
    pub args: Py<PyList>,
    // Map from expected values hash to the actual value
    pub load_map: Py<PyDict>,
    // Map from value hash to the expected value
    pub dump_map: Py<PyDict>,
    pub items_repr: String,
}

#[pymethods]
impl LiteralType {
    #[new]
    #[pyo3(signature = (args, custom_encoder=None))]
    fn new(
        args: &Bound<'_, PyList>,
        custom_encoder: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<(Self, BaseType)> {
        let len = args.len();
        let load_map = PyDict::new(args.py());
        let dump_map = PyDict::new(args.py());
        let mut items_repr = Vec::with_capacity(len);

        for py_value in args.iter() {
            // Get enum value or use the value itself
            let value = match py_value.getattr(intern!(py_value.py(), "value")) {
                Ok(value) => value,
                Err(_) => py_value.clone(),
            };

            dump_map.set_item(&py_value, value.clone().unbind())?;
            load_map.set_item(&value, &py_value)?;
            items_repr.push(fmt_py(&value));

            // For support fast load with try_cast_from_string option enabled
            // we need to add additional mapping for string values
            if let Ok(value) = value.downcast::<PyInt>() {
                let str_value = value.str().unwrap();
                load_map.set_item((&str_value, false), &py_value)?;
            }
        }

        Ok((
            LiteralType {
                args: args.clone().unbind(),
                items_repr: format!("[{}]", items_repr.join(", ")),
                load_map: load_map.unbind(),
                dump_map: dump_map.unbind(),
            },
            BaseType::new(custom_encoder),
        ))
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

#[pyclass(frozen, extends=BaseType, module="serpyco_rs", subclass)]
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
        name: &Bound<'_, PyAny>,
        state_key: &Bound<'_, PyAny>,
        meta: &Bound<'_, PyAny>,
        custom_encoder: Option<&Bound<'_, PyAny>>,
    ) -> (Self, BaseType) {
        (
            RecursionHolder {
                name: name.clone().unbind(),
                state_key: state_key.clone().unbind(),
                meta: meta.clone().unbind(),
            },
            BaseType::new(custom_encoder),
        )
    }

    pub fn get_inner_type<'a>(&'a self, py: Python<'a>) -> PyResult<Bound<'a, pyo3::PyAny>> {
        match self.meta.bind(py).get_item(&self.state_key) {
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

#[pyclass(frozen, extends=BaseType, module="serpyco_rs")]
#[derive(Debug, Clone)]
pub struct CustomType {
    #[pyo3(get)]
    json_schema: Py<PyAny>,
}

#[pymethods]
impl CustomType {
    #[new]
    fn new(custom_encoder: &Bound<'_, PyAny>, json_schema: &Bound<'_, PyAny>) -> (Self, BaseType) {
        (
            CustomType {
                json_schema: json_schema.clone().unbind(),
            },
            BaseType::new(Some(custom_encoder)),
        )
    }

    fn __eq__(self_: PyRef<'_, Self>, other: PyRef<'_, Self>, py: Python<'_>) -> PyResult<bool> {
        let base = self_.as_ref();
        let base_other = other.as_ref();
        Ok(base.__eq__(base_other, py)? && py_eq!(self_.json_schema, other.json_schema, py))
    }

    fn __repr__(&self) -> String {
        "<CustomType>".to_string()
    }
}
