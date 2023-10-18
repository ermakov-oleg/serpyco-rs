use pyo3::{PyErr, PyResult, Python};
use pyo3_ffi::PyObject;
use std::fmt::{Display, Formatter};

use super::py_types::ObjectType;
use crate::python::macros::{call_method, ffi};
use crate::python::types::ITEMS_STR;
use crate::python::{
    from_ptr_or_err, from_ptr_or_opt, obj_to_str, py_len, py_object_get_item, py_str_to_str,
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

    /// Is Bytes value.
    #[inline]
    pub fn is_bytes(&self) -> bool {
        self.object_type == ObjectType::Bytes
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

    /// Checks if the value is a integer.
    #[inline]
    pub fn is_int(&self) -> bool {
        self.object_type == ObjectType::Int
    }

    /// Checks if the value is a float or integer.
    #[inline]
    pub fn is_number(&self) -> bool {
        self.object_type == ObjectType::Int || self.object_type == ObjectType::Float
    }

    /// Checks if the value is a string.
    #[inline]
    pub fn is_string(&self) -> bool {
        self.object_type == ObjectType::Str
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

    pub fn str_len(&self) -> PyResult<isize> {
        if self.object_type == ObjectType::Str {
            py_len(self.py_object)
        } else {
            Err(PyErr::new::<pyo3::exceptions::PyTypeError, _>(
                "Not a string",
            ))
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

    /// Represents object as sequence.
    #[inline]
    pub fn as_sequence(&self) -> Option<SequenceImpl> {
        if ffi!(PySequence_Check(self.py_object)) != 0 && self.object_type != ObjectType::Str {
            Some(SequenceImpl::new(self.py_object))
        } else {
            None
        }
    }

    #[inline]
    pub fn maybe_number(&self) -> Option<f64> {
        match self.object_type {
            ObjectType::Float => self.as_float(),
            ObjectType::Int => self.as_int().map(|i| i as f64),
            ObjectType::Str => self.as_str().and_then(|s| s.parse::<f64>().ok()),
            _ => None,
        }
    }
}

impl From<Value> for i64 {
    #[inline]
    fn from(value: Value) -> Self {
        value.as_int().expect("Failed to convert Value to i64")
    }
}

impl From<Value> for f64 {
    #[inline]
    fn from(value: Value) -> Self {
        value
            .as_float()
            .or(value.as_int().map(|i| i as f64))
            .expect("Failed to convert Value to f64")
    }
}

/// Represents a Python immutable sequence.
pub trait Sequence {
    /// Returns the pointer to the underlying PyObject.
    fn as_ptr(&self) -> *mut pyo3::ffi::PyObject;
    /// Returns the length of the sequence.
    fn len(&self) -> isize;
    /// Map the sequence to a new sequence with the given function.
    /// The function must return a new PyObject or increase the reference count of the given PyObject.
    fn map_into<T>(
        &self,
        f: &dyn Fn(isize, *mut pyo3::ffi::PyObject) -> PyResult<*mut pyo3::ffi::PyObject>,
    ) -> PyResult<T>
    where
        T: MutableSequence;
}

/// Represents a Python mutable sequence.
pub trait MutableSequence {
    /// Creates a new empty sequence with the given capacity.
    fn new_with_capacity(capacity: isize) -> Self;
    /// Sets the value at the given index.
    fn set(&mut self, index: isize, value: *mut pyo3::ffi::PyObject);
}

/// Represents a Python array.
/// This is a wrapper around a PyObject pointer.
pub struct Array {
    py_object: *mut pyo3::ffi::PyObject,
    len: isize,
}

impl Array {
    /// Creates a new array from the given PyObject.
    #[inline]
    pub fn new(py_object: *mut pyo3::ffi::PyObject) -> Self {
        Array {
            py_object,
            len: ffi!(PyList_GET_SIZE(py_object)),
        }
    }

    /// Returns the value at the given index.
    /// Will panic if the index is out of bounds.
    #[inline]
    pub fn get_item(&self, index: isize) -> Value {
        let item = ffi!(PyList_GET_ITEM(self.py_object, index)); // rc not changed
        Value::new(item)
    }
}

impl Sequence for Array {
    /// Returns the pointer to the underlying PyObject.
    #[inline]
    fn as_ptr(&self) -> *mut PyObject {
        self.py_object
    }
    /// Returns the length of the array.
    #[inline]
    fn len(&self) -> isize {
        self.len
    }

    fn map_into<T>(
        &self,
        f: &dyn Fn(isize, *mut PyObject) -> PyResult<*mut PyObject>,
    ) -> PyResult<T>
    where
        T: MutableSequence,
    {
        let mut result = T::new_with_capacity(self.len);
        for i in 0..self.len {
            let item = ffi!(PyList_GET_ITEM(self.as_ptr(), i));
            let new_item = f(i, item)?;
            result.set(i, new_item);
        }
        Ok(result)
    }
}

impl MutableSequence for Array {
    /// Creates a new empty array with the given capacity.
    #[inline]
    fn new_with_capacity(capacity: isize) -> Self {
        let py_object = ffi!(PyList_New(capacity));
        Array {
            py_object,
            len: capacity,
        }
    }
    /// Sets the value at the given index.
    #[inline]
    fn set(&mut self, index: isize, value: *mut pyo3::ffi::PyObject) {
        ffi!(PyList_SetItem(self.py_object, index, value));
    }
}

/// Represents a Python sequence.
/// This is a wrapper around a PyObject pointer.
pub struct SequenceImpl {
    py_object: *mut pyo3::ffi::PyObject,
    len: isize,
}

impl Sequence for SequenceImpl {
    #[inline]
    fn as_ptr(&self) -> *mut pyo3::ffi::PyObject {
        self.py_object
    }

    #[inline]
    fn len(&self) -> isize {
        self.len
    }

    #[inline]
    fn map_into<T>(
        &self,
        f: &dyn Fn(isize, *mut pyo3::ffi::PyObject) -> PyResult<*mut pyo3::ffi::PyObject>,
    ) -> PyResult<T>
    where
        T: MutableSequence,
    {
        let mut result = T::new_with_capacity(self.len);
        for i in 0..self.len {
            let item = ffi!(PySequence_GetItem(self.as_ptr(), i)); // RC +1
            let new_item = f(i, item)?;
            result.set(i, new_item);
            ffi!(Py_DECREF(item));
        }
        Ok(result)
    }
}

impl SequenceImpl {
    /// Creates a new sequence from the given PyObject.
    #[inline]
    pub fn new(py_object: *mut pyo3::ffi::PyObject) -> Self {
        SequenceImpl {
            py_object,
            len: py_len(py_object).expect("Failed to get sequence length"),
        }
    }
}

/// Represents a Python tuple.
/// This is a wrapper around a PyObject pointer.
pub struct Tuple {
    py_object: *mut pyo3::ffi::PyObject,
}

impl Tuple {
    #[inline]
    pub fn as_ptr(&self) -> *mut pyo3::ffi::PyObject {
        self.py_object
    }
}

impl MutableSequence for Tuple {
    #[inline]
    fn new_with_capacity(capacity: isize) -> Self {
        let py_object = ffi!(PyTuple_New(capacity));
        Tuple { py_object }
    }

    #[inline]
    fn set(&mut self, index: isize, value: *mut PyObject) {
        ffi!(PyTuple_SetItem(self.py_object, index, value));
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

impl Display for Value {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self.as_str() {
            Some(val) => write!(f, "{}", val),
            None => write!(f, "{}", _to_string(self.py_object)),
        }
    }
}

impl Display for Tuple {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", _to_string(self.py_object))
    }
}

impl Display for SequenceImpl {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", _to_string(self.py_object))
    }
}

#[inline]
pub fn _to_string(py_object: *mut pyo3::ffi::PyObject) -> &'static str {
    if let Ok(result) = obj_to_str(py_object) {
        if let Ok(result) = py_str_to_str(result) {
            return result;
        }
    }
    "<Failed to convert PyObject to &str>"
}
