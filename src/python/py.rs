use super::macros::{call_method, ffi};
use super::types::{
    DATE_STR, DECIMAL_PY_TYPE, FALSE, ISOFORMAT_STR, ITEMS_STR, NONE_PY_TYPE, NOT_SET,
    PY_OBJECT__NEW__, TRUE, UUID_PY_TYPE, VALUE_STR,
};
use crate::python::macros::use_immortal;
use pyo3::{ffi, AsPyPointer, PyAny, PyErr, PyResult, Python};
use pyo3_ffi::Py_ssize_t;
use serde_json::Number;
use std::os::raw::c_int;
use std::ptr::NonNull;

#[inline]
pub(crate) fn get_none() -> *mut ffi::PyObject {
    use_immortal!(NONE_PY_TYPE)
}

#[inline]
pub(crate) fn to_decimal(value: *mut ffi::PyObject) -> PyResult<*mut ffi::PyObject> {
    py_object_call1_make_tuple_or_err(unsafe { DECIMAL_PY_TYPE }, value)
}

#[inline]
pub(crate) fn to_uuid(value: *mut ffi::PyObject) -> PyResult<*mut ffi::PyObject> {
    py_object_call1_make_tuple_or_err(unsafe { UUID_PY_TYPE }, value)
}

#[inline]
pub(crate) fn to_bool(value: bool) -> *mut ffi::PyObject {
    if value {
        use_immortal!(TRUE)
    } else {
        use_immortal!(FALSE)
    }
}

#[inline]
pub(crate) fn get_value_attr(value: *mut ffi::PyObject) -> PyResult<*mut ffi::PyObject> {
    py_object_get_attr(value, unsafe { VALUE_STR })
}

#[inline]
pub(crate) fn py_len(obj: *mut ffi::PyObject) -> PyResult<Py_ssize_t> {
    let v = ffi!(PyObject_Size(obj));
    if v == -1 {
        Err(Python::with_gil(PyErr::fetch))
    } else {
        Ok(v)
    }
}

#[inline]
pub(crate) fn is_not_set(obj: &PyAny) -> PyResult<bool> {
    Ok(obj.as_ptr() == unsafe { NOT_SET })
}

#[inline]
pub(crate) fn is_none(obj: *mut ffi::PyObject) -> bool {
    obj == unsafe { NONE_PY_TYPE }
}

#[inline]
pub(crate) fn is_datetime(obj: *mut ffi::PyObject) -> bool {
    ffi!(PyDateTime_Check(obj)) == 1
}

#[inline]
pub(crate) fn datetime_to_date(obj: *mut ffi::PyObject) -> PyResult<*mut ffi::PyObject> {
    call_method!(obj, DATE_STR)
}

#[inline]
pub(crate) fn call_isoformat(value: *mut ffi::PyObject) -> PyResult<*mut ffi::PyObject> {
    call_method!(value, ISOFORMAT_STR)
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

#[inline(always)]
fn parse_i64(val: i64) -> *mut ffi::PyObject {
    ffi!(PyLong_FromLongLong(val))
}

#[inline(always)]
fn parse_u64(val: u64) -> *mut ffi::PyObject {
    ffi!(PyLong_FromUnsignedLongLong(val))
}

#[inline(always)]
fn parse_f64(val: f64) -> *mut ffi::PyObject {
    ffi!(PyFloat_FromDouble(val))
}

#[inline(always)]
pub(crate) fn parse_number(val: Number) -> *mut ffi::PyObject {
    if val.is_f64() {
        parse_f64(val.as_f64().unwrap())
    } else if val.is_i64() {
        parse_i64(val.as_i64().unwrap())
    } else {
        parse_u64(val.as_u64().unwrap())
    }
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
    // Obj RC +1
    from_ptr_or_err(ffi!(PyObject_GetItem(obj, key)))
}

#[inline]
pub(crate) fn iter_over_dict_items(obj: *mut ffi::PyObject) -> PyResult<PyObjectIterator> {
    let items = call_method!(obj, ITEMS_STR)?;
    to_iter(items)
}

#[inline]
fn to_iter(obj: *mut ffi::PyObject) -> PyResult<PyObjectIterator> {
    let internal = PyObjectIterator(from_ptr_or_err(ffi!(PyObject_GetIter(obj)))?);
    Ok(internal)
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
fn from_ptr_or_opt(ptr: *mut ffi::PyObject) -> Option<*mut ffi::PyObject> {
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