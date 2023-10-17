use pyo3::ffi::PyObject;
use pyo3::types::PyModule;
use pyo3::Python;
use pyo3::{AsPyPointer, Py, PyAny, PyResult};
use std::any::Any;
use std::os::raw::c_char;
use std::sync::Once;

use super::py::py_object_get_attr;

pub static mut BYTES_TYPE: *mut PyObject = 0 as *mut PyObject;
pub static mut ENUM_TYPE: *mut PyObject = 0 as *mut PyObject;
pub static mut OPTIONAL_TYPE: *mut PyObject = 0 as *mut PyObject;
pub static mut ARRAY_TYPE: *mut PyObject = 0 as *mut PyObject;
pub static mut DICTIONARY_TYPE: *mut PyObject = 0 as *mut PyObject;
pub static mut TUPLE_TYPE: *mut PyObject = 0 as *mut PyObject;
pub static mut ANY_TYPE: *mut PyObject = 0 as *mut PyObject;
pub static mut RECURSION_HOLDER_TYPE: *mut PyObject = 0 as *mut PyObject;
pub static mut UNION_TYPE: *mut PyObject = 0 as *mut PyObject;
pub static mut LITERAL_TYPE: *mut PyObject = 0 as *mut PyObject;
pub static mut ITEMS_STR: *mut PyObject = 0 as *mut PyObject;
pub static mut ISOFORMAT_STR: *mut PyObject = 0 as *mut PyObject;
pub static mut DATE_STR: *mut PyObject = 0 as *mut PyObject;
pub static mut VALUE_STR: *mut PyObject = 0 as *mut PyObject;
pub static mut UUID_PY_TYPE: *mut PyObject = 0 as *mut PyObject;
pub static mut NONE_PY_TYPE: *mut PyObject = 0 as *mut PyObject;
pub static mut DECIMAL_PY_TYPE: *mut PyObject = 0 as *mut PyObject;
pub static mut PY_TUPLE_0: *mut PyObject = 0 as *mut PyObject;
pub static mut PY_OBJECT__NEW__: *mut PyObject = 0 as *mut PyObject;
pub static mut EMPTY_UNICODE: *mut PyObject = 0 as *mut PyObject;
pub static mut TRUE: *mut PyObject = 0 as *mut PyObject;
pub static mut FALSE: *mut PyObject = 0 as *mut PyObject;

static INIT: Once = Once::new();

#[derive(Clone, Debug)]
pub enum Type<Base = Option<BaseType>> {
    Integer(IntegerType, Base),
    Float(FloatType, Base),
    Decimal(DecimalType, Base),
    String(StringType, Base),
    Boolean(BooleanType, Base),
    Uuid(UUIDType, Base),
    Bytes(Py<PyAny>),
    Time(TimeType, Base),
    DateTime(DateTimeType, Base),
    Date(DateType, Base),
    Entity(EntityType, Base, usize),
    TypedDict(TypedDictType, Base, usize),
    Enum(Py<PyAny>),
    Optional(Py<PyAny>),
    Array(Py<PyAny>),
    Dictionary(Py<PyAny>),
    Tuple(Py<PyAny>),
    UnionType(Py<PyAny>),
    LiteralType(Py<PyAny>),
    RecursionHolder(Py<PyAny>),
    Any(Py<PyAny>),
}
use crate::validator::types::{BaseType, BooleanType, DateTimeType, DateType, DecimalType, EntityType, FloatType, IntegerType, StringType, TimeType, TypedDictType, UUIDType};

pub fn get_object_type(type_info: &PyAny) -> PyResult<Type> {
    let base_type = type_info.extract::<BaseType>();
    if let Err(e) = &base_type {
        // todo: Raise error, after all types are implemented
        println!("base_type: {:?}", e);
    }
    let base_type = base_type.ok();

    if let Ok(t) = type_info.extract::<IntegerType>() {
        Ok(Type::Integer(t, base_type))
    } else if let Ok(t) = type_info.extract::<StringType>() {
        Ok(Type::String(t, base_type))
    } else if let Ok(t) = type_info.extract::<FloatType>() {
        Ok(Type::Float(t, base_type))
    } else if let Ok(t) = type_info.extract::<DecimalType>() {
        Ok(Type::Decimal(t, base_type))
    } else if let Ok(t) = type_info.extract::<BooleanType>() {
        Ok(Type::Boolean(t, base_type))
    } else if let Ok(t) = type_info.extract::<UUIDType>() {
        Ok(Type::Uuid(t, base_type))
    } else if let Ok(t) = type_info.extract::<TimeType>() {
        Ok(Type::Time(t, base_type))
    } else if let Ok(t) = type_info.extract::<DateTimeType>() {
        Ok(Type::DateTime(t, base_type))
    } else if let Ok(t) = type_info.extract::<DateType>() {
        Ok(Type::Date(t, base_type))
    } else if check_type!(type_info, ENUM_TYPE) {
        Ok(Type::Enum(type_info.into()))
    } else if let Ok(t) = type_info.extract::<EntityType>() {
        let python_object_id = type_info.as_ptr() as *const _ as usize;
        Ok(Type::Entity(t, base_type, python_object_id))
    } else if let Ok(t) = type_info.extract::<TypedDictType>() {
        let python_object_id = type_info.as_ptr() as *const _ as usize;
        Ok(Type::TypedDict(t, base_type, python_object_id))
    } else if check_type!(type_info, OPTIONAL_TYPE) {
        Ok(Type::Optional(type_info.into()))
    } else if check_type!(type_info, ARRAY_TYPE) {
        Ok(Type::Array(type_info.into()))
    } else if check_type!(type_info, DICTIONARY_TYPE) {
        Ok(Type::Dictionary(type_info.into()))
    } else if check_type!(type_info, TUPLE_TYPE) {
        Ok(Type::Tuple(type_info.into()))
    } else if check_type!(type_info, ANY_TYPE) {
        Ok(Type::Any(type_info.into()))
    } else if check_type!(type_info, RECURSION_HOLDER_TYPE) {
        Ok(Type::RecursionHolder(type_info.into()))
    } else if check_type!(type_info, UNION_TYPE) {
        Ok(Type::UnionType(type_info.into()))
    } else if check_type!(type_info, LITERAL_TYPE) {
        Ok(Type::LiteralType(type_info.into()))
    } else if check_type!(type_info, BYTES_TYPE) {
        Ok(Type::Bytes(type_info.into()))
    } else {
        todo!("py Error 'Unsupported type' {type_info}")
    }
}

pub fn init(py: Python<'_>) {
    INIT.call_once(|| unsafe {
        if pyo3_ffi::PyDateTimeAPI().is_null() {
            // initialize datetime module
            pyo3_ffi::PyDateTime_IMPORT()
        };
        let describe = PyModule::import(py, "serpyco_rs._describe_types").unwrap();
        BYTES_TYPE = get_attr_ptr!(describe, "BytesType");
        ENUM_TYPE = get_attr_ptr!(describe, "EnumType");
        OPTIONAL_TYPE = get_attr_ptr!(describe, "OptionalType");
        ARRAY_TYPE = get_attr_ptr!(describe, "ArrayType");
        DICTIONARY_TYPE = get_attr_ptr!(describe, "DictionaryType");
        TUPLE_TYPE = get_attr_ptr!(describe, "TupleType");
        RECURSION_HOLDER_TYPE = get_attr_ptr!(describe, "RecursionHolder");
        UNION_TYPE = get_attr_ptr!(describe, "UnionType");
        LITERAL_TYPE = get_attr_ptr!(describe, "LiteralType");
        ANY_TYPE = get_attr_ptr!(describe, "AnyType");

        let uuid = PyModule::import(py, "uuid").unwrap();
        UUID_PY_TYPE = get_attr_ptr!(uuid, "UUID");

        let builtins = PyModule::import(py, "builtins").unwrap();
        NONE_PY_TYPE = get_attr_ptr!(builtins, "None");

        let object = get_attr_ptr!(builtins, "object");
        let new_str = pyo3_ffi::PyUnicode_InternFromString("__new__\0".as_ptr() as *const c_char);
        PY_OBJECT__NEW__ = py_object_get_attr(object, new_str).unwrap();

        let decimal_str =
            pyo3_ffi::PyUnicode_InternFromString("Decimal\0".as_ptr() as *const c_char);
        let decimal = PyModule::import(py, "decimal").unwrap();
        DECIMAL_PY_TYPE = py_object_get_attr(decimal.as_ptr(), decimal_str).unwrap();

        ITEMS_STR = pyo3_ffi::PyUnicode_InternFromString("items\0".as_ptr() as *const c_char);
        VALUE_STR = pyo3_ffi::PyUnicode_InternFromString("value\0".as_ptr() as *const c_char);
        ISOFORMAT_STR =
            pyo3_ffi::PyUnicode_InternFromString("isoformat\0".as_ptr() as *const c_char);
        DATE_STR = pyo3_ffi::PyUnicode_InternFromString("date\0".as_ptr() as *const c_char);

        TRUE = pyo3_ffi::Py_True();
        FALSE = pyo3_ffi::Py_False();

        EMPTY_UNICODE = pyo3_ffi::PyUnicode_New(0, 255);
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
