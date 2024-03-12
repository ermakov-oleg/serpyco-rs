use std::os::raw::c_int;
use std::ptr::NonNull;

use pyo3::{ffi, PyErr, PyResult, Python};
use pyo3::prelude::*;
use pyo3::types::{PyDict, PyList};
use pyo3_ffi::Py_ssize_t;

use crate::python::macros::use_immortal;

use super::macros::ffi;
use super::types::{
    DECIMAL_PY_TYPE, NONE_PY_TYPE, PY_OBJECT__NEW__, PY_OBJECT__SETATTR__,
    UUID_PY_TYPE, VALUE_STR,
};

#[inline]
pub(crate) fn get_none() -> *mut ffi::PyObject {
    use_immortal!(NONE_PY_TYPE)
}

#[inline]
pub(crate) fn to_decimal(value: *mut ffi::PyObject) -> PyResult<*mut ffi::PyObject> {
    let result = py_object_call1_make_tuple_or_err(unsafe { DECIMAL_PY_TYPE }, obj_to_str(value)?);
    if ffi!(PyUnicode_CheckExact(value)) == 1 {
        ffi!(Py_DECREF(value));
    }
    result
}

#[inline]
pub(crate) fn to_uuid(value: *mut ffi::PyObject) -> PyResult<*mut ffi::PyObject> {
    py_object_call1_make_tuple_or_err(unsafe { UUID_PY_TYPE }, value)
}

#[inline]
pub(crate) fn to_int(value: *mut pyo3::ffi::PyObject) -> PyResult<i64> {
    let result = ffi!(PyLong_AsLongLong(value));
    if result == -1 && !ffi!(PyErr_Occurred()).is_null() {
        Err(Python::with_gil(PyErr::fetch))
    } else {
        Ok(result)
    }
}

#[inline]
pub(crate) fn to_float(value: *mut pyo3::ffi::PyObject) -> PyResult<f64> {
    let result = ffi!(PyFloat_AsDouble(value));
    if result == -1.0 && !ffi!(PyErr_Occurred()).is_null() {
        Err(Python::with_gil(PyErr::fetch))
    } else {
        Ok(result)
    }
}

#[inline]
pub(crate) fn get_value_attr(value: *mut ffi::PyObject) -> PyResult<*mut ffi::PyObject> {
    py_object_get_attr(value, unsafe { VALUE_STR })
}

#[inline]
pub(crate) fn py_len(obj: *mut ffi::PyObject) -> PyResult<isize> {
    let v = ffi!(PyObject_Size(obj));
    if v == -1 {
        Err(Python::with_gil(PyErr::fetch))
    } else {
        Ok(v)
    }
}


#[inline]
pub(crate) fn create_py_list(py: Python, size: usize) -> Bound<PyList> {
    let size: Py_ssize_t = size.try_into().expect("size is too large");
    unsafe {
        Bound::from_owned_ptr(py, ffi::PyList_New(size)).downcast_into_unchecked()
    }
}


#[inline]
pub(crate) fn py_list_get_item<'a>(list: &'a Bound<'a, PyList>, index: usize) -> Bound<'a, PyAny> {
    #[cfg(any(Py_LIMITED_API, PyPy))]
    let item = list.get_item(index).expect("list.get failed");
    #[cfg(not(any(Py_LIMITED_API, PyPy)))]
    let item = unsafe { list.get_item_unchecked(index) };
    item
}

#[inline]
pub(crate) fn py_list_set_item(list: &Bound<PyList>, index: usize, value: Bound<PyAny>)  {
    let index: Py_ssize_t = index.try_into().expect("size is too large");
    #[cfg(not(Py_LIMITED_API))]
    ffi!(PyList_SET_ITEM(list.as_ptr(), index, value.into_ptr()));
    #[cfg(Py_LIMITED_API)]
    ffi!(PyList_SetItem(list.as_ptr(), index, value.into_ptr()));
}


#[inline]
pub(crate) fn py_dict_set_item(list: &Bound<PyDict>, key: *mut ffi::PyObject, value: Bound<PyAny>) -> PyResult<()>  {
    let result = ffi!(PyDict_SetItem(list.as_ptr(), key, value.as_ptr()));
    error_on_minusone(result)
}

#[inline]
pub(crate) fn create_new_object(cls: *mut ffi::PyObject) -> PyResult<*mut ffi::PyObject> {
    let tuple_arg = from_ptr_or_err(ffi!(PyTuple_Pack(1, cls)))?;
    let result = py_object_call1_or_err(unsafe { PY_OBJECT__NEW__ }, tuple_arg);
    ffi!(Py_DECREF(tuple_arg));
    result
}

#[inline]
pub(crate) fn obj_to_str(obj: *mut ffi::PyObject) -> PyResult<*mut ffi::PyObject> {
    from_ptr_or_err(ffi!(PyObject_Str(obj)))
}

#[inline]
fn py_object_call1_or_err(
    obj: *mut ffi::PyObject,
    args: *mut ffi::PyObject,
) -> PyResult<*mut ffi::PyObject> {
    from_ptr_or_err(ffi!(PyObject_CallObject(obj, args)))
}

#[inline]
pub(crate) fn py_object_call1_make_tuple_or_err(
    obj: *mut ffi::PyObject,
    arg: *mut ffi::PyObject,
) -> PyResult<*mut ffi::PyObject> {
    let tuple_arg = from_ptr_or_err(ffi!(PyTuple_Pack(1, arg)))?;
    let result = py_object_call1_or_err(obj, tuple_arg)?;
    ffi!(Py_DECREF(tuple_arg));
    Ok(result)
}

#[inline]
pub(crate) fn py_object_get_attr(
    obj: *mut ffi::PyObject,
    attr_name: *mut ffi::PyObject,
) -> PyResult<*mut ffi::PyObject> {
    from_ptr_or_err(ffi!(PyObject_GetAttr(obj, attr_name)))
}

#[inline]
pub(crate) fn py_object_set_attr(
    obj: *mut ffi::PyObject,
    attr_name: *mut ffi::PyObject,
    value: *mut ffi::PyObject,
) -> PyResult<()> {
    let ret = ffi!(PyObject_SetAttr(obj, attr_name, value));
    error_on_minusone(ret)
}

#[inline]
pub(crate) fn py_frozen_object_set_attr(
    obj: *mut ffi::PyObject,
    attr_name: *mut ffi::PyObject,
    value: *mut ffi::PyObject,
) -> PyResult<()> {
    let tuple_arg = from_ptr_or_err(ffi!(PyTuple_Pack(3, obj, attr_name, value)))?;
    py_object_call1_or_err(unsafe { PY_OBJECT__SETATTR__ }, tuple_arg)?;
    ffi!(Py_DECREF(tuple_arg));
    Ok(())
}

#[inline]
pub(crate) fn py_str_to_str(obj: *mut ffi::PyObject) -> PyResult<&'static str> {
    let utf8_slice = {
        cfg_if::cfg_if! {
            if #[cfg(any(Py_3_10, not(Py_LIMITED_API)))] {
                // PyUnicode_AsUTF8AndSize only available on limited API starting with 3.10.
                let mut size: ffi::Py_ssize_t = 0;
                let data = ffi!(PyUnicode_AsUTF8AndSize(obj, &mut size));
                if data.is_null() {
                    return Err(Python::with_gil(PyErr::fetch));
                } else {
                    unsafe { std::slice::from_raw_parts(data as *const u8, size as usize) }
                }
            } else {
                let ptr = from_ptr_or_err(ffi!(PyUnicode_AsUTF8String(obj)))?;
                unsafe {
                    let buffer = ffi!(PyBytes_AsString(ptr)) as *const u8;
                    let length = ffi!(PyBytes_Size(ptr)) as usize;
                    debug_assert!(!buffer.is_null());
                    std::slice::from_raw_parts(buffer, length)
                }
            }
        }
    };
    Ok(unsafe { std::str::from_utf8_unchecked(utf8_slice) })
}

#[inline]
pub(crate) fn py_tuple_get_item(
    obj: *mut ffi::PyObject,
    index: usize,
) -> PyResult<*mut ffi::PyObject> {
    // Doesn't touch RC
    from_ptr_or_err(ffi!(PyTuple_GetItem(obj, index as Py_ssize_t)))
}

#[inline]
pub(crate) fn py_object_get_item(
    obj: *mut ffi::PyObject,
    key: *mut ffi::PyObject,
) -> PyResult<*mut ffi::PyObject> {
    // todo: use PyDict_GetItemWithError
    // Obj RC +1
    from_ptr_or_err(ffi!(PyObject_GetItem(obj, key)))
}

/// Returns None if key not found
/// without setting an exception
#[inline]
pub(crate) fn py_dict_get_item(
    obj: *mut ffi::PyObject,
    key: *mut ffi::PyObject,
) -> Option<*mut ffi::PyObject> {
    // Obj RC not changed
    from_ptr_or_opt(ffi!(PyDict_GetItemWithError(obj, key)))
}

pub(crate) struct PyObjectIterator(*mut ffi::PyObject);

impl Iterator for PyObjectIterator {
    type Item = PyResult<*mut ffi::PyObject>;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        match from_ptr_or_opt(ffi!(PyIter_Next(self.0))) {
            Some(item) => Some(Ok(item)),
            None => Python::with_gil(|py| PyErr::take(py).map(Err)),
        }
    }
}

#[inline]
pub(crate) fn from_ptr_or_opt(ptr: *mut ffi::PyObject) -> Option<*mut ffi::PyObject> {
    NonNull::new(ptr).map(|p| p.as_ptr())
}

#[inline]
pub(crate) fn from_ptr_or_err(ptr: *mut ffi::PyObject) -> PyResult<*mut ffi::PyObject> {
    from_ptr_or_opt(ptr).ok_or_else(|| Python::with_gil(PyErr::fetch))
}

#[inline]
pub(crate) fn error_on_minusone(result: c_int) -> PyResult<()> {
    if result != -1 {
        Ok(())
    } else {
        Err(Python::with_gil(PyErr::fetch))
    }
}
