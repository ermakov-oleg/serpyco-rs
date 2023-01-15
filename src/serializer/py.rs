use crate::serializer::macros::{call_method, ffi};
use crate::serializer::types::{DECIMAL_PY_TYPE, ITEMS_STR, NOT_SET, PY_OBJECT__NEW__};
use pyo3::{ffi, AsPyPointer, PyAny, PyErr, PyResult, Python};
use pyo3_ffi::Py_ssize_t;
use std::ffi::CString;
use std::os::raw::{c_char, c_int};
use std::ptr::NonNull;

#[inline]
pub fn to_decimal(value: *mut ffi::PyObject) -> PyResult<*mut ffi::PyObject> {
    py_object_call1_make_tuple_or_err(unsafe { DECIMAL_PY_TYPE }, value)
}

#[inline]
pub fn py_len(obj: *mut ffi::PyObject) -> PyResult<Py_ssize_t> {
    let v = ffi!(PyObject_Size(obj));
    if v == -1 {
        Err(Python::with_gil(PyErr::fetch))
    } else {
        Ok(v)
    }
}

#[inline]
pub fn is_not_set(obj: &PyAny) -> PyResult<bool> {
    Ok(obj.as_ptr() == unsafe { NOT_SET })
}

#[inline]
pub fn create_new_object(cls: *mut ffi::PyObject) -> PyResult<*mut ffi::PyObject> {
    let tuple_arg = from_ptr_or_err(ffi!(PyTuple_Pack(1, cls)))?;
    let result = py_object_call1_or_err(unsafe { PY_OBJECT__NEW__ }, tuple_arg);
    ffi!(Py_DECREF(tuple_arg));
    result
}

#[inline]
pub fn obj_to_str(obj: *mut ffi::PyObject) -> PyResult<*mut ffi::PyObject> {
    from_ptr_or_err(ffi!(PyObject_Str(obj)))
}

pub fn to_py_string(s: &str) -> *mut ffi::PyObject {
    let c_str = CString::new(s).unwrap();
    let c_world: *const c_char = c_str.as_ptr() as *const c_char;
    ffi!(PyUnicode_InternFromString(c_world))
}

#[inline]
fn py_object_call1_or_err(
    obj: *mut ffi::PyObject,
    args: *mut ffi::PyObject,
) -> PyResult<*mut ffi::PyObject> {
    from_ptr_or_err(ffi!(PyObject_CallObject(obj, args)))
}

#[inline]
pub fn py_object_call1_make_tuple_or_err(
    obj: *mut ffi::PyObject,
    arg: *mut ffi::PyObject,
) -> PyResult<*mut ffi::PyObject> {
    let tuple_arg = from_ptr_or_err(ffi!(PyTuple_Pack(1, arg)))?;
    let result = py_object_call1_or_err(obj, tuple_arg)?;
    ffi!(Py_DECREF(tuple_arg));
    Ok(result)
}

#[inline]
pub fn py_object_get_attr(
    obj: *mut ffi::PyObject,
    attr_name: *mut ffi::PyObject,
) -> PyResult<*mut ffi::PyObject> {
    from_ptr_or_err(ffi!(PyObject_GetAttr(obj, attr_name)))
}

#[inline]
pub fn py_object_set_attr(
    obj: *mut ffi::PyObject,
    attr_name: *mut ffi::PyObject,
    value: *mut ffi::PyObject,
) -> PyResult<()> {
    let ret = ffi!(PyObject_SetAttr(obj, attr_name, value));
    error_on_minusone(ret)
}

#[inline]
pub fn py_str_to_str(obj: *mut ffi::PyObject) -> PyResult<&'static str> {
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
pub fn py_tuple_get_item(obj: *mut ffi::PyObject, index: usize) -> PyResult<*mut ffi::PyObject> {
    from_ptr_or_err(ffi!(PyTuple_GetItem(obj, index as Py_ssize_t)))
}

#[inline]
pub fn py_object_get_item(
    obj: *mut ffi::PyObject,
    key: *mut ffi::PyObject,
) -> PyResult<*mut ffi::PyObject> {
    from_ptr_or_err(ffi!(PyObject_GetItem(obj, key)))
}

#[inline]
pub fn iter_over_dict_items(obj: *mut ffi::PyObject) -> PyResult<PyObjectIterator> {
    let items = call_method!(obj, ITEMS_STR)?;
    to_iter(items)
}

#[inline]
fn to_iter(obj: *mut ffi::PyObject) -> PyResult<PyObjectIterator> {
    let internal = PyObjectIterator(from_ptr_or_err(ffi!(PyObject_GetIter(obj)))?);
    Ok(internal)
}

pub struct PyObjectIterator(*mut ffi::PyObject);

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
fn from_ptr_or_opt(ptr: *mut ffi::PyObject) -> Option<*mut ffi::PyObject> {
    NonNull::new(ptr).map(|p| p.as_ptr())
}

#[inline]
pub fn from_ptr_or_err(ptr: *mut ffi::PyObject) -> PyResult<*mut ffi::PyObject> {
    from_ptr_or_opt(ptr).ok_or_else(|| Python::with_gil(PyErr::fetch))
}

#[inline]
pub fn error_on_minusone(result: c_int) -> PyResult<()> {
    if result != -1 {
        Ok(())
    } else {
        Err(Python::with_gil(PyErr::fetch))
    }
}
