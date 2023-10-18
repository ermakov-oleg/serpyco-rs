use std::ffi::CStr;
use std::{os::raw::c_char, sync::Once};

use pyo3::ffi::{
    PyDict_New, PyFloat_FromDouble, PyList_New, PyLong_FromLongLong, PyObject, PyTypeObject,
    PyUnicode_New, Py_None, Py_True,
};
use pyo3_ffi::PyBytes_FromStringAndSize;
use pyo3_ffi::Py_TYPE;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ObjectType {
    Str,
    Int,
    Bool,
    None,
    Float,
    List,
    Dict,
    Bytes,
    Unknown(String),
}

pub fn get_object_type_from_object(object: *mut pyo3::ffi::PyObject) -> ObjectType {
    unsafe {
        let object_type = Py_TYPE(object);
        get_object_type(object_type)
    }
}

#[inline]
pub fn get_object_type(object_type: *mut pyo3::ffi::PyTypeObject) -> ObjectType {
    if object_type == unsafe { STR_TYPE } {
        ObjectType::Str
    } else if object_type == unsafe { FLOAT_TYPE } {
        ObjectType::Float
    } else if object_type == unsafe { BOOL_TYPE } {
        ObjectType::Bool
    } else if object_type == unsafe { INT_TYPE } {
        ObjectType::Int
    } else if object_type == unsafe { NONE_TYPE } {
        ObjectType::None
    } else if object_type == unsafe { LIST_TYPE } {
        ObjectType::List
    } else if object_type == unsafe { DICT_TYPE } {
        ObjectType::Dict
    } else if object_type == unsafe { BYTES_TYPE } {
        ObjectType::Bytes
    } else {
        ObjectType::Unknown(get_type_name(object_type).to_string())
    }
}

pub fn get_type_name(object_type: *mut pyo3::ffi::PyTypeObject) -> std::borrow::Cow<'static, str> {
    unsafe { CStr::from_ptr((*object_type).tp_name).to_string_lossy() }
}

pub static mut TRUE: *mut pyo3::ffi::PyObject = 0 as *mut pyo3::ffi::PyObject;

pub static mut STR_TYPE: *mut PyTypeObject = 0 as *mut PyTypeObject;
pub static mut INT_TYPE: *mut PyTypeObject = 0 as *mut PyTypeObject;
pub static mut BOOL_TYPE: *mut PyTypeObject = 0 as *mut PyTypeObject;
pub static mut NONE_TYPE: *mut PyTypeObject = 0 as *mut PyTypeObject;
pub static mut FLOAT_TYPE: *mut PyTypeObject = 0 as *mut PyTypeObject;
pub static mut LIST_TYPE: *mut PyTypeObject = 0 as *mut PyTypeObject;
pub static mut DICT_TYPE: *mut PyTypeObject = 0 as *mut PyTypeObject;
pub static mut BYTES_TYPE: *mut PyTypeObject = 0 as *mut PyTypeObject;
pub static mut VALUE_STR: *mut PyObject = 0 as *mut PyObject;

static INIT: Once = Once::new();

/// Set empty type object pointers with their actual values.
/// We need these Python-side type objects for direct comparison during conversion to inner types
/// NOTE. This function should be called before any serialization logic
pub fn init() {
    INIT.call_once(|| unsafe {
        TRUE = Py_True();
        STR_TYPE = Py_TYPE(PyUnicode_New(0, 255));
        DICT_TYPE = Py_TYPE(PyDict_New());
        LIST_TYPE = Py_TYPE(PyList_New(0_isize));
        NONE_TYPE = Py_TYPE(Py_None());
        BOOL_TYPE = Py_TYPE(TRUE);
        INT_TYPE = Py_TYPE(PyLong_FromLongLong(0));
        FLOAT_TYPE = Py_TYPE(PyFloat_FromDouble(0.0));
        BYTES_TYPE = Py_TYPE(PyBytes_FromStringAndSize(std::ptr::null_mut(), 0_isize));
        VALUE_STR = pyo3::ffi::PyUnicode_InternFromString("value\0".as_ptr().cast::<c_char>());
    });
}
