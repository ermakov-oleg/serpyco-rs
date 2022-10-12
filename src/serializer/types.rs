use pyo3::ffi::PyObject;
use pyo3::types::PyModule;
use pyo3::Python;
use pyo3::{AsPyPointer, Py, PyAny, PyResult};
use std::sync::Once;

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
pub static mut ITEMS_STR: *mut PyObject = 0 as *mut PyObject;
pub static mut VALUE_STR: *mut PyObject = 0 as *mut PyObject;
pub static mut UUID_PY_TYPE: *mut PyObject = 0 as *mut PyObject;
pub static mut NONE_PY_TYPE: *mut PyObject = 0 as *mut PyObject;
pub static mut PY_TUPLE_0: *mut PyObject = 0 as *mut PyObject;

static INIT: Once = Once::new();

#[derive(Clone, Debug)]
pub enum Type {
    IntegerType(Py<PyAny>),
    StringType(Py<PyAny>),
    BytesType(Py<PyAny>),
    FloatType(Py<PyAny>),
    DecimalType(Py<PyAny>),
    BooleanType(Py<PyAny>),
    UUIDType(Py<PyAny>),
    TimeType(Py<PyAny>),     // todo: implement
    DateTimeType(Py<PyAny>), // todo: implement
    DateType(Py<PyAny>),     // todo: implement
    EnumType(Py<PyAny>),
    EntityType(Py<PyAny>),
    OptionalType(Py<PyAny>),
    ArrayType(Py<PyAny>),
    DictionaryType(Py<PyAny>),
    TupleType(Py<PyAny>), // todo: implement
    AnyType(Py<PyAny>),
}

pub fn get_object_type(type_info: &PyAny) -> PyResult<Type> {
    if check_type!(type_info, INTEGER_TYPE) {
        Ok(Type::IntegerType(type_info.into()))
    } else if check_type!(type_info, STRING_TYPE) {
        Ok(Type::StringType(type_info.into()))
    } else if check_type!(type_info, BYTES_TYPE) {
        Ok(Type::BytesType(type_info.into()))
    } else if check_type!(type_info, FLOAT_TYPE) {
        Ok(Type::FloatType(type_info.into()))
    } else if check_type!(type_info, DECIMAL_TYPE) {
        Ok(Type::DecimalType(type_info.into()))
    } else if check_type!(type_info, BOOLEAN_TYPE) {
        Ok(Type::BooleanType(type_info.into()))
    } else if check_type!(type_info, UUID_TYPE) {
        Ok(Type::UUIDType(type_info.into()))
    } else if check_type!(type_info, TIME_TYPE) {
        Ok(Type::TimeType(type_info.into()))
    } else if check_type!(type_info, DATETIME_TYPE) {
        Ok(Type::DateTimeType(type_info.into()))
    } else if check_type!(type_info, DATE_TYPE) {
        Ok(Type::DateType(type_info.into()))
    } else if check_type!(type_info, ENUM_TYPE) {
        Ok(Type::EnumType(type_info.into()))
    } else if check_type!(type_info, ENTITY_TYPE) {
        Ok(Type::EntityType(type_info.into()))
    } else if check_type!(type_info, OPTIONAL_TYPE) {
        Ok(Type::OptionalType(type_info.into()))
    } else if check_type!(type_info, ARRAY_TYPE) {
        Ok(Type::ArrayType(type_info.into()))
    } else if check_type!(type_info, DICTIONARY_TYPE) {
        Ok(Type::DictionaryType(type_info.into()))
    } else if check_type!(type_info, TUPLE_TYPE) {
        Ok(Type::TupleType(type_info.into()))
    } else if check_type!(type_info, ANY_TYPE) {
        Ok(Type::AnyType(type_info.into()))
    } else {
        todo!("py Error 'Unsupported type'")
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
        ANY_TYPE = get_attr_ptr!(describe, "AnyType");

        let uuid = PyModule::import(py, "uuid").unwrap();
        UUID_PY_TYPE = get_attr_ptr!(uuid, "UUID");

        let builtins = PyModule::import(py, "builtins").unwrap();
        NONE_PY_TYPE = get_attr_ptr!(builtins, "None");

        ITEMS_STR = to_py_string("items");
        VALUE_STR = to_py_string("value");

        PY_TUPLE_0 = ffi!(PyTuple_New(0));
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

use crate::serializer::py::to_py_string;
pub(crate) use check_type;
pub(crate) use get_attr_ptr;

use super::macros::ffi;
