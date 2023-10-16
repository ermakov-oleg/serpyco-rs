use crate::jsonschema::ser::{get_type_name, ObjectType};
use crate::jsonschema::types;
use pyo3_ffi::Py_TYPE;

pub fn get_object_type_from_object(object: *mut pyo3::ffi::PyObject) -> ObjectType {
    unsafe {
        let object_type = Py_TYPE(object);
        get_object_type(object_type)
    }
}

#[inline]
pub fn get_object_type(object_type: *mut pyo3::ffi::PyTypeObject) -> ObjectType {
    if object_type == unsafe { types::STR_TYPE } {
        ObjectType::Str
    } else if object_type == unsafe { types::FLOAT_TYPE } {
        ObjectType::Float
    } else if object_type == unsafe { types::BOOL_TYPE } {
        ObjectType::Bool
    } else if object_type == unsafe { types::INT_TYPE } {
        ObjectType::Int
    } else if object_type == unsafe { types::NONE_TYPE } {
        ObjectType::None
    } else if object_type == unsafe { types::LIST_TYPE } {
        ObjectType::List
    } else if object_type == unsafe { types::DICT_TYPE } {
        ObjectType::Dict
    } else {
        ObjectType::Unknown(get_type_name(object_type).to_string())
    }
}
