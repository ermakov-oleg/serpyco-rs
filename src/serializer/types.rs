use pyo3::ffi::PyObject;
use pyo3::types::PyModule;
use pyo3::Python;
use pyo3::{AsPyPointer, Py, PyAny, PyResult};
use std::sync::Once;

use crate::serializer::py::{py_object_get_attr, to_py_string};

pub static mut INTEGER_TYPE: *mut PyObject = 0 as *mut PyObject;
pub static mut STRING_TYPE: *mut PyObject = 0 as *mut PyObject;
pub static mut BYTES_TYPE: *mut PyObject = 0 as *mut PyObject;
pub static mut FLOAT_TYPE: *mut PyObject = 0 as *mut PyObject;
pub static mut DECIMAL_TYPE: *mut PyObject = 0 as *mut PyObject;
pub static mut BOOLEAN_TYPE: *mut PyObject = 0 as *mut PyObject;
pub static mut UUID_TYPE: *mut PyObject = 0 as *mut PyObject;
pub static mut TIME_TYPE: *mut PyObject = 0 as *mut PyObject;
pub static mut DATETIME_TYPE: *mut PyObject = 0 as *mut PyObject;
pub static mut DATE_TYPE: *mut PyObject = 0 as *mut PyObject;
pub static mut ENUM_TYPE: *mut PyObject = 0 as *mut PyObject;
pub static mut ENTITY_TYPE: *mut PyObject = 0 as *mut PyObject;
pub static mut OPTIONAL_TYPE: *mut PyObject = 0 as *mut PyObject;
pub static mut ARRAY_TYPE: *mut PyObject = 0 as *mut PyObject;
pub static mut DICTIONARY_TYPE: *mut PyObject = 0 as *mut PyObject;
pub static mut TUPLE_TYPE: *mut PyObject = 0 as *mut PyObject;
pub static mut ANY_TYPE: *mut PyObject = 0 as *mut PyObject;
pub static mut RECURSION_HOLDER_TYPE: *mut PyObject = 0 as *mut PyObject;
pub static mut NOT_SET: *mut PyObject = 0 as *mut PyObject;
pub static mut ITEMS_STR: *mut PyObject = 0 as *mut PyObject;
pub static mut ISOFORMAT_STR: *mut PyObject = 0 as *mut PyObject;
pub static mut VALUE_STR: *mut PyObject = 0 as *mut PyObject;
pub static mut UUID_PY_TYPE: *mut PyObject = 0 as *mut PyObject;
pub static mut NONE_PY_TYPE: *mut PyObject = 0 as *mut PyObject;
pub static mut DECIMAL_PY_TYPE: *mut PyObject = 0 as *mut PyObject;
pub static mut PY_TUPLE_0: *mut PyObject = 0 as *mut PyObject;
pub static mut PY_OBJECT__NEW__: *mut PyObject = 0 as *mut PyObject;

static INIT: Once = Once::new();

#[derive(Clone, Debug)]
pub enum Type {
    Integer,
    String,
    Bytes,
    Float,
    Decimal,
    Boolean,
    Uuid,
    Time,
    DateTime,
    Date,
    Enum(Py<PyAny>),
    Entity(Py<PyAny>),
    Optional(Py<PyAny>),
    Array(Py<PyAny>),
    Dictionary(Py<PyAny>),
    Tuple(Py<PyAny>),
    RecursionHolder(Py<PyAny>),
    Any,
}

pub fn get_object_type(type_info: &PyAny) -> PyResult<Type> {
    if check_type!(type_info, INTEGER_TYPE) {
        Ok(Type::Integer)
    } else if check_type!(type_info, STRING_TYPE) {
        Ok(Type::String)
    } else if check_type!(type_info, BYTES_TYPE) {
        Ok(Type::Bytes)
    } else if check_type!(type_info, FLOAT_TYPE) {
        Ok(Type::Float)
    } else if check_type!(type_info, DECIMAL_TYPE) {
        Ok(Type::Decimal)
    } else if check_type!(type_info, BOOLEAN_TYPE) {
        Ok(Type::Boolean)
    } else if check_type!(type_info, UUID_TYPE) {
        Ok(Type::Uuid)
    } else if check_type!(type_info, TIME_TYPE) {
        Ok(Type::Time)
    } else if check_type!(type_info, DATETIME_TYPE) {
        Ok(Type::DateTime)
    } else if check_type!(type_info, DATE_TYPE) {
        Ok(Type::Date)
    } else if check_type!(type_info, ENUM_TYPE) {
        Ok(Type::Enum(type_info.into()))
    } else if check_type!(type_info, ENTITY_TYPE) {
        Ok(Type::Entity(type_info.into()))
    } else if check_type!(type_info, OPTIONAL_TYPE) {
        Ok(Type::Optional(type_info.into()))
    } else if check_type!(type_info, ARRAY_TYPE) {
        Ok(Type::Array(type_info.into()))
    } else if check_type!(type_info, DICTIONARY_TYPE) {
        Ok(Type::Dictionary(type_info.into()))
    } else if check_type!(type_info, TUPLE_TYPE) {
        Ok(Type::Tuple(type_info.into()))
    } else if check_type!(type_info, ANY_TYPE) {
        Ok(Type::Any)
    } else if check_type!(type_info, RECURSION_HOLDER_TYPE) {
        Ok(Type::RecursionHolder(type_info.into()))
    } else {
        todo!("py Error 'Unsupported type' {type_info}")
    }
}

pub fn init(py: Python<'_>) {
    INIT.call_once(|| unsafe {
        let describe = PyModule::import(py, "serpyco_rs._describe").unwrap();
        INTEGER_TYPE = get_attr_ptr!(describe, "IntegerType");
        STRING_TYPE = get_attr_ptr!(describe, "StringType");
        BYTES_TYPE = get_attr_ptr!(describe, "BytesType");
        FLOAT_TYPE = get_attr_ptr!(describe, "FloatType");
        DECIMAL_TYPE = get_attr_ptr!(describe, "DecimalType");
        BOOLEAN_TYPE = get_attr_ptr!(describe, "BooleanType");
        UUID_TYPE = get_attr_ptr!(describe, "UUIDType");
        TIME_TYPE = get_attr_ptr!(describe, "TimeType");
        DATETIME_TYPE = get_attr_ptr!(describe, "DateTimeType");
        DATE_TYPE = get_attr_ptr!(describe, "DateType");
        ENUM_TYPE = get_attr_ptr!(describe, "EnumType");
        ENTITY_TYPE = get_attr_ptr!(describe, "EntityType");
        OPTIONAL_TYPE = get_attr_ptr!(describe, "OptionalType");
        ARRAY_TYPE = get_attr_ptr!(describe, "ArrayType");
        DICTIONARY_TYPE = get_attr_ptr!(describe, "DictionaryType");
        TUPLE_TYPE = get_attr_ptr!(describe, "TupleType");
        RECURSION_HOLDER_TYPE = get_attr_ptr!(describe, "RecursionHolder");
        NOT_SET = get_attr_ptr!(describe, "NOT_SET");

        let uuid = PyModule::import(py, "uuid").unwrap();
        UUID_PY_TYPE = get_attr_ptr!(uuid, "UUID");

        let builtins = PyModule::import(py, "builtins").unwrap();
        NONE_PY_TYPE = get_attr_ptr!(builtins, "None");

        let object = get_attr_ptr!(builtins, "object");
        PY_OBJECT__NEW__ = py_object_get_attr(object, to_py_string("__new__")).unwrap();

        let decimal = PyModule::import(py, "decimal").unwrap();
        DECIMAL_PY_TYPE = py_object_get_attr(decimal.as_ptr(), to_py_string("Decimal")).unwrap();

        ITEMS_STR = to_py_string("items");
        VALUE_STR = to_py_string("value");
        ISOFORMAT_STR = to_py_string("isoformat");

        PY_TUPLE_0 = pyo3_ffi::PyTuple_New(0);
    });
}

macro_rules! check_type {
    ($py_obj:ident, $type:expr) => {
        $py_obj.get_type().as_ptr() == unsafe { $type }
    };
}

macro_rules! get_attr_ptr {
    ($mod:expr, $type:expr) => {
        $mod.getattr($type).unwrap().as_ptr()
    };
}

pub(crate) use check_type;
pub(crate) use get_attr_ptr;
