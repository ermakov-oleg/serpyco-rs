use pyo3::{PyErr, PyResult, Python};

use crate::jsonschema::ser::ObjectType;
use crate::python::macros::{call_method, ffi};
use crate::python::types::ITEMS_STR;
use crate::python::{
    from_ptr_or_err, from_ptr_or_opt, obj_to_str, py_object_get_item, py_str_to_str,
    py_tuple_get_item,
};

use super::py_types::get_object_type_from_object;

/// Represents a Python value.
/// This is a wrapper around a PyObject pointer.
pub struct Value {
    py_object: *mut pyo3::ffi::PyObject,
    pub object_type: ObjectType,
}

impl Value {
    /// Creates a new value from the given PyObject.
    #[inline]
    pub fn new(py_object: *mut pyo3::ffi::PyObject) -> Self {
        Value {
            py_object,
            object_type: get_object_type_from_object(py_object),
        }
    }
}

impl Value {
    /// Returns the pointer to the underlying PyObject.
    #[inline]
    pub fn as_ptr(&self) -> *mut pyo3::ffi::PyObject {
        self.py_object
    }

    /// Is None value.
    #[inline]
    pub fn is_none(&self) -> bool {
        self.object_type == ObjectType::None
    }

    /// Represents as Bool value.
    #[inline]
    pub fn as_bool(&self) -> Option<bool> {
        if self.object_type == ObjectType::Bool {
            Some(self.py_object == unsafe { pyo3::ffi::Py_True() })
        } else {
            None
        }
    }

    /// Represents as Int value.
    #[inline]
    pub fn as_int(&self) -> Option<i64> {
        if self.object_type == ObjectType::Int {
            Some(ffi!(PyLong_AsLongLong(self.py_object)))
        } else {
            None
        }
    }

    /// Represents as Float value.
    #[inline]
    pub fn as_float(&self) -> Option<f64> {
        if self.object_type == ObjectType::Float {
            Some(ffi!(PyFloat_AS_DOUBLE(self.py_object)))
        } else {
            None
        }
    }

    /// Represents as Str value.
    #[inline]
    pub fn as_str(&self) -> Option<&str> {
        if self.object_type == ObjectType::Str {
            let slice = py_str_to_str(self.py_object).expect("Failed to convert PyStr to &str");
            Some(slice)
        } else {
            None
        }
    }

    /// Represents as Array value.
    #[inline]
    pub fn as_array(&self) -> Option<Array> {
        if self.object_type == ObjectType::List {
            Some(Array::new(self.py_object))
        } else {
            None
        }
    }

    /// Represents as Dict value.
    #[inline]
    pub fn as_dict(&self) -> Option<Dict> {
        if self.object_type == ObjectType::Dict {
            Some(Dict::new(self.py_object))
        } else {
            None
        }
    }

    /// Represents object as a string.
    #[inline]
    pub fn to_string(&self) -> PyResult<&'static str> {
        let result = obj_to_str(self.py_object)?;
        py_str_to_str(result)
    }
}

/// Represents a Python array.
/// This is a wrapper around a PyObject pointer.
pub struct Array {
    py_object: *mut pyo3::ffi::PyObject,
}

impl Array {
    /// Creates a new array from the given PyObject.
    #[inline]
    pub fn new(py_object: *mut pyo3::ffi::PyObject) -> Self {
        Array { py_object }
    }

    /// Creates a new empty array with the given capacity.
    #[inline]
    pub fn new_with_capacity(capacity: isize) -> Self {
        let py_object = ffi!(PyList_New(capacity));
        Array { py_object }
    }
}

impl Array {
    /// Returns the pointer to the underlying PyObject.
    #[inline]
    pub fn as_ptr(&self) -> *mut pyo3::ffi::PyObject {
        self.py_object
    }

    /// Returns the length of the array.
    #[inline]
    pub fn len(&self) -> isize {
        ffi!(PyList_GET_SIZE(self.py_object)) // rc not changed
    }

    /// Returns the value at the given index.
    /// Will panic if the index is out of bounds.
    #[inline]
    pub fn get_item(&self, index: isize) -> Value {
        let item = ffi!(PyList_GET_ITEM(self.py_object, index)); // rc not changed
        Value::new(item)
    }

    /// Sets the value at the given index.
    #[inline]
    pub fn set(&mut self, index: isize, value: *mut pyo3::ffi::PyObject) {
        ffi!(PyList_SetItem(self.py_object, index, value));
    }
}

/// Represents a Python dict.
/// This is a wrapper around a PyObject pointer.
pub struct Dict {
    py_object: *mut pyo3::ffi::PyObject,
}

impl Dict {
    /// Creates a new dict from the given PyObject.
    pub fn new(py_object: *mut pyo3::ffi::PyObject) -> Self {
        Dict { py_object }
    }

    pub fn new_empty() -> Self {
        let py_object = ffi!(PyDict_New());
        Dict { py_object }
    }
}

impl Dict {
    /// Returns the pointer to the underlying PyObject.
    #[inline]
    pub fn as_ptr(&self) -> *mut pyo3::ffi::PyObject {
        self.py_object
    }

    /// Returns value of the given key.
    #[inline]
    pub fn get_item(&self, key: *mut pyo3::ffi::PyObject) -> Option<Value> {
        let item = py_object_get_item(self.py_object, key);
        if let Ok(item) = item {
            return Some(Value::new(item));
        }
        None
    }

    /// Sets the value at the given key.
    #[inline]
    pub fn set(&mut self, key: *mut pyo3::ffi::PyObject, value: *mut pyo3::ffi::PyObject) {
        ffi!(PyDict_SetItem(self.py_object, key, value)); // key and val RC +1
        ffi!(Py_DECREF(key));
        ffi!(Py_DECREF(value));
    }

    /// Returns dict items iterator.
    #[inline]
    pub fn iter(&self) -> PyResult<PyObjectIterator> {
        let items = call_method!(self.py_object, ITEMS_STR)?;
        let internal = PyObjectIterator(from_ptr_or_err(ffi!(PyObject_GetIter(items)))?);
        Ok(internal)
    }
}

pub struct PyObjectIterator(*mut pyo3::ffi::PyObject);

impl Iterator for PyObjectIterator {
    type Item = PyResult<(Value, Value)>;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        match from_ptr_or_opt(ffi!(PyIter_Next(self.0))) {
            Some(item) => {
                let key = match py_tuple_get_item(item, 0) {
                    Ok(key) => Value::new(key),
                    Err(err) => return Some(Err(err)),
                };
                let value = match py_tuple_get_item(item, 1) {
                    Ok(value) => Value::new(value),
                    Err(err) => return Some(Err(err)),
                };
                ffi!(Py_DECREF(item));
                Some(Ok((key, value)))
            }
            None => Python::with_gil(|py| PyErr::take(py).map(Err)),
        }
    }
}
