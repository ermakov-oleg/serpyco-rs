use std::fmt::{Debug, Formatter};

use pyo3::{AsPyPointer, PyResult};

use crate::jsonschema::ser::ObjectType;
use crate::python::macros::ffi;
use crate::python::{obj_to_str, py_object_get_item, py_str_to_str};

use super::py_types::get_object_type_from_object;

pub struct Value {
    py_object: *mut pyo3::ffi::PyObject,
    pub object_type: ObjectType,
}

impl Value {
    pub fn new(py_object: *mut pyo3::ffi::PyObject) -> Self {
        Value {
            py_object,
            object_type: get_object_type_from_object(py_object),
        }
    }
}

impl Value {
    pub fn is_valid_type(&self) -> bool {
        !matches!(self.object_type, ObjectType::Unknown(_))
    }

    pub fn as_ptr(&self) -> *mut pyo3::ffi::PyObject {
        self.py_object
    }

    /// Is None value.
    pub fn is_none(&self) -> bool {
        self.object_type == ObjectType::None
    }

    /// Represents as Bool value.
    pub fn as_bool(&self) -> Option<bool> {
        if self.object_type == ObjectType::Bool {
            Some(self.py_object == unsafe { pyo3::ffi::Py_True() })
        } else {
            None
        }
    }

    /// Represents as Int value.
    pub fn as_int(&self) -> Option<i64> {
        if self.object_type == ObjectType::Int {
            Some(ffi!(PyLong_AsLongLong(self.py_object)))
        } else {
            None
        }
    }

    /// Represents as Float value.
    pub fn as_float(&self) -> Option<f64> {
        if self.object_type == ObjectType::Float {
            Some(ffi!(PyFloat_AS_DOUBLE(self.py_object)))
        } else {
            None
        }
    }

    /// Represents as Str value.
    pub fn as_str(&self) -> Option<&str> {
        if self.object_type == ObjectType::Str {
            let slice = py_str_to_str(self.py_object).expect("Failed to convert PyStr to &str");
            Some(slice)
        } else {
            None
        }
    }

    /// Represents as Array value.
    pub fn as_array(&self) -> Option<Array> {
        if self.object_type == ObjectType::List {
            Some(Array::new(self.py_object))
        } else {
            None
        }
    }

    pub fn as_dict(&self) -> Option<Dict> {
        if self.object_type == ObjectType::Dict {
            Some(Dict::new(self.py_object))
        } else {
            None
        }
    }

    /// Represents object as a string.
    pub fn to_string(&self) -> PyResult<&'static str> {
        let result = obj_to_str(self.py_object)?;
        py_str_to_str(result)
    }
}

pub struct Array {
    py_object: *mut pyo3::ffi::PyObject,
}

impl Array {
    pub fn new(py_object: *mut pyo3::ffi::PyObject) -> Self {
        Array { py_object }
    }
}

impl Array {
    /// Returns the length of the array.
    #[inline]
    pub fn len(&self) -> usize {
        ffi!(PyList_GET_SIZE(self.py_object)) as usize
    }

    /// Returns the value at the given index.
    /// This method will return None if the index is out of bounds.
    #[inline]
    pub fn get(&self, index: usize) -> Option<Value> {
        if index >= self.len() {
            return None;
        }
        let item = ffi!(PyList_GET_ITEM(self.py_object, index as isize));
        Some(Value::new(item))
    }
}

pub struct Dict {
    py_object: *mut pyo3::ffi::PyObject,
}

impl Dict {
    pub fn new(py_object: *mut pyo3::ffi::PyObject) -> Self {
        Dict { py_object }
    }
}

impl Dict {
    pub fn get(&self, key: *mut pyo3::ffi::PyObject,) -> Option<Value> {
        let item = py_object_get_item(self.py_object, key);
        if let Ok(item) = item {
            return Some(Value::new(item));
        }
        None
    }

    pub fn keys(&self) -> Vec<Value> {
        // let keys = self.py_object.keys();
        let result = Vec::new();
        // for key in keys {
        //     result.push(Value::new(key))
        // }
        result
    }
}

impl Debug for Value {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let val = match &self.object_type {
            ObjectType::Str => self.as_str().unwrap().to_string(),
            ObjectType::Int => format!("{}", self.as_int().unwrap()),
            ObjectType::Bool => format!("{}", self.as_bool().unwrap()),
            ObjectType::Float => format!("{}", self.as_float().unwrap()),
            ObjectType::List => format!("{:?}", self.as_array().unwrap()),
            ObjectType::Dict => format!("{:?}", self.as_dict().unwrap()),
            _ => "Invalid value".to_string(),
        };
        f.debug_struct("Value")
            .field("object_type", &self.object_type)
            .field("val", &val)
            .finish()
    }
}

impl Debug for Array {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut val = Vec::new();
        for i in 0..self.len() {
            let item = self.get(i).unwrap();
            val.push(format!("{:?}", item));
        }
        f.debug_struct("PyArray").field("val", &val).finish()
    }
}

impl Debug for Dict {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PyArray").finish()
        // f.debug_map().entries(
        //     self.keys().iter().map(|key| (format!("{:?}", key), format!("{:?}", self.get(key.py_object).unwrap())))
        // ).finish()
    }
}
