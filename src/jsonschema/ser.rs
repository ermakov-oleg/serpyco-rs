// Taken from jsonschema-rs
use pyo3::{
    exceptions,
    ffi::{
        PyFloat_AS_DOUBLE, PyList_GET_ITEM, PyList_GET_SIZE, PyLong_AsLongLong, PyObject_GetAttr,
        PyTuple_GET_ITEM, PyTuple_GET_SIZE, Py_TYPE,
    },
    prelude::*,
    types::PyAny,
    AsPyPointer,
};
use serde::{
    ser::{self, Serialize, SerializeMap, SerializeSeq},
    Serializer,
};

use super::{ffi, types};
use crate::python::py_str_to_str;
use std::ffi::CStr;

pub const RECURSION_LIMIT: u8 = 255;

#[derive(Clone)]
pub enum ObjectType {
    Str,
    Int,
    Bool,
    None,
    Float,
    List,
    Dict,
    Tuple,
    Enum,
    Bytes,
    Unknown(String),
}

struct SerializePyObject {
    object: *mut pyo3::ffi::PyObject,
    object_type: ObjectType,
    recursion_depth: u8,
    pass_through_bytes: bool,
}

impl SerializePyObject {
    #[inline]
    pub fn new(
        object: *mut pyo3::ffi::PyObject,
        recursion_depth: u8,
        pass_through_bytes: bool,
    ) -> Self {
        SerializePyObject {
            object,
            object_type: get_object_type_from_object(object),
            recursion_depth,
            pass_through_bytes,
        }
    }

    #[inline]
    pub const fn with_obtype(
        object: *mut pyo3::ffi::PyObject,
        object_type: ObjectType,
        recursion_depth: u8,
        pass_through_bytes: bool,
    ) -> Self {
        SerializePyObject {
            object,
            object_type,
            recursion_depth,
            pass_through_bytes,
        }
    }
}

#[inline]
fn is_enum_subclass(object_type: *mut pyo3::ffi::PyTypeObject) -> bool {
    unsafe { (*(object_type.cast::<ffi::PyTypeObject>())).ob_type == types::ENUM_TYPE }
}

fn get_object_type_from_object(object: *mut pyo3::ffi::PyObject) -> ObjectType {
    unsafe {
        let object_type = Py_TYPE(object);
        get_object_type(object_type)
    }
}

fn get_type_name(object_type: *mut pyo3::ffi::PyTypeObject) -> std::borrow::Cow<'static, str> {
    unsafe { CStr::from_ptr((*object_type).tp_name).to_string_lossy() }
}

#[inline]
fn check_type_is_str<E: ser::Error>(object: *mut pyo3::ffi::PyObject) -> Result<(), E> {
    let object_type = unsafe { Py_TYPE(object) };
    if object_type != unsafe { types::STR_TYPE } {
        return Err(ser::Error::custom(format!(
            "Dict key must be str. Got '{}'",
            get_type_name(object_type)
        )));
    }
    Ok(())
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
    } else if object_type == unsafe { types::TUPLE_TYPE } {
        ObjectType::Tuple
    } else if object_type == unsafe { types::DICT_TYPE } {
        ObjectType::Dict
    } else if object_type == unsafe { types::BYTES_TYPE } {
        ObjectType::Bytes
    } else if is_enum_subclass(object_type) {
        ObjectType::Enum
    } else {
        ObjectType::Unknown(get_type_name(object_type).to_string())
    }
}

/// Convert a Python value to `serde_json::Value`
impl Serialize for SerializePyObject {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match self.object_type {
            ObjectType::Str => {
                let slice = py_str_to_str(self.object).expect("Failed to convert PyStr to &str");
                serializer.serialize_str(slice)
            }
            ObjectType::Int => serializer.serialize_i64(unsafe { PyLong_AsLongLong(self.object) }),
            ObjectType::Float => {
                serializer.serialize_f64(unsafe { PyFloat_AS_DOUBLE(self.object) })
            }
            ObjectType::Bool => serializer.serialize_bool(self.object == unsafe { types::TRUE }),
            ObjectType::None => serializer.serialize_unit(),
            ObjectType::Bytes if self.pass_through_bytes => serializer.serialize_str(""),
            ObjectType::Dict => {
                if self.recursion_depth == RECURSION_LIMIT {
                    return Err(ser::Error::custom("Recursion limit reached"));
                }
                let length = unsafe { pyo3::ffi::PyDict_Size(self.object) } as usize;
                if length == 0 {
                    serializer.serialize_map(Some(0))?.end()
                } else {
                    let mut map = serializer.serialize_map(Some(length))?;
                    let mut pos = 0_isize;
                    let mut key: *mut pyo3::ffi::PyObject = std::ptr::null_mut();
                    let mut value: *mut pyo3::ffi::PyObject = std::ptr::null_mut();
                    for _ in 0..length {
                        unsafe {
                            pyo3::ffi::PyDict_Next(self.object, &mut pos, &mut key, &mut value);
                        }
                        check_type_is_str(key)?;
                        let slice = py_str_to_str(key).expect("Failed to convert PyStr to &str");
                        #[allow(clippy::arithmetic_side_effects)]
                        map.serialize_entry(
                            slice,
                            &SerializePyObject::new(
                                value,
                                self.recursion_depth + 1,
                                self.pass_through_bytes,
                            ),
                        )?;
                    }
                    map.end()
                }
            }
            ObjectType::List => {
                if self.recursion_depth == RECURSION_LIMIT {
                    return Err(ser::Error::custom("Recursion limit reached"));
                }
                let length = unsafe { PyList_GET_SIZE(self.object) as usize };
                if length == 0 {
                    serializer.serialize_seq(Some(0))?.end()
                } else {
                    let mut type_ptr = std::ptr::null_mut();
                    let mut ob_type = ObjectType::Str;
                    let mut sequence = serializer.serialize_seq(Some(length))?;
                    for i in 0..length {
                        let elem = unsafe { PyList_GET_ITEM(self.object, i as isize) };
                        let current_ob_type = unsafe { Py_TYPE(elem) };
                        if current_ob_type != type_ptr {
                            type_ptr = current_ob_type;
                            ob_type = get_object_type(current_ob_type);
                        }
                        #[allow(clippy::arithmetic_side_effects)]
                        sequence.serialize_element(&SerializePyObject::with_obtype(
                            elem,
                            ob_type.clone(),
                            self.recursion_depth + 1,
                            self.pass_through_bytes,
                        ))?;
                    }
                    sequence.end()
                }
            }
            ObjectType::Tuple => {
                if self.recursion_depth == RECURSION_LIMIT {
                    return Err(ser::Error::custom("Recursion limit reached"));
                }
                let length = unsafe { PyTuple_GET_SIZE(self.object) as usize };
                if length == 0 {
                    serializer.serialize_seq(Some(0))?.end()
                } else {
                    let mut type_ptr = std::ptr::null_mut();
                    let mut ob_type = ObjectType::Str;
                    let mut sequence = serializer.serialize_seq(Some(length))?;
                    for i in 0..length {
                        let elem = unsafe { PyTuple_GET_ITEM(self.object, i as isize) };
                        let current_ob_type = unsafe { Py_TYPE(elem) };
                        if current_ob_type != type_ptr {
                            type_ptr = current_ob_type;
                            ob_type = get_object_type(current_ob_type);
                        }
                        #[allow(clippy::arithmetic_side_effects)]
                        sequence.serialize_element(&SerializePyObject::with_obtype(
                            elem,
                            ob_type.clone(),
                            self.recursion_depth + 1,
                            self.pass_through_bytes,
                        ))?;
                    }
                    sequence.end()
                }
            }
            ObjectType::Enum => {
                let value = unsafe { PyObject_GetAttr(self.object, types::VALUE_STR) };
                #[allow(clippy::arithmetic_side_effects)]
                SerializePyObject::new(value, self.recursion_depth + 1, self.pass_through_bytes)
                    .serialize(serializer)
            }
            ObjectType::Bytes => Err(ser::Error::custom("Bytes are not supported")),
            ObjectType::Unknown(ref type_name) => Err(ser::Error::custom(format!(
                "Unsupported type: '{}'",
                type_name
            ))),
        }
    }
}

#[inline]
pub(crate) fn to_value(object: &PyAny, pass_through_bytes: bool) -> PyResult<serde_json::Value> {
    serde_json::to_value(SerializePyObject::new(
        object.as_ptr(),
        0,
        pass_through_bytes,
    ))
    .map_err(|err| exceptions::PyValueError::new_err(err.to_string()))
}
