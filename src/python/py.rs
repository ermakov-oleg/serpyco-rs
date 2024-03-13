use std::os::raw::c_int;
use std::ptr::NonNull;

use pyo3::prelude::*;
use pyo3::types::{PyDict, PyList, PyString, PyTuple};
use pyo3::{ffi, PyErr, PyResult, Python};
use pyo3_ffi::Py_ssize_t;

use super::macros::ffi;
use super::types::{PY_OBJECT__SETATTR__};

#[inline]
pub(crate) fn to_uuid<'a, 'py>(
    uuid_cls: &'a Bound<'py, PyAny>,
    value: &'a Bound<'py, PyString>,
) -> PyResult<Bound<'py, PyAny>> {
    uuid_cls.call1((value,))
}

#[inline]
pub(crate) fn create_py_list(py: Python, size: usize) -> Bound<PyList> {
    let size: Py_ssize_t = size.try_into().expect("size is too large");
    unsafe { Bound::from_owned_ptr(py, ffi::PyList_New(size)).downcast_into_unchecked() }
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
pub(crate) fn py_list_set_item(list: &Bound<PyList>, index: usize, value: Bound<PyAny>) {
    let index: Py_ssize_t = index.try_into().expect("size is too large");
    #[cfg(not(Py_LIMITED_API))]
    ffi!(PyList_SET_ITEM(list.as_ptr(), index, value.into_ptr()));
    #[cfg(Py_LIMITED_API)]
    ffi!(PyList_SetItem(list.as_ptr(), index, value.into_ptr()));
}

#[inline]
pub(crate) fn create_py_dict_known_size(py: Python, size: usize) -> Bound<PyDict> {
    let size: Py_ssize_t = size.try_into().expect("size is too large");
    unsafe { Bound::from_owned_ptr(py, ffi::_PyDict_NewPresized(size)).downcast_into_unchecked() }
}

#[inline]
pub(crate) fn py_dict_set_item(
    list: &Bound<PyDict>,
    key: *mut ffi::PyObject,
    value: Bound<PyAny>,
) -> PyResult<()> {
    // todo: Check performance
    let result = ffi!(PyDict_SetItem(list.as_ptr(), key, value.as_ptr()));
    error_on_minusone(result)
}

#[inline]
pub(crate) fn create_py_tuple(py: Python, size: usize) -> Bound<PyTuple> {
    let size: Py_ssize_t = size.try_into().expect("size is too large");
    unsafe { Bound::from_owned_ptr(py, ffi::PyTuple_New(size)).downcast_into_unchecked() }
}

#[inline]
pub(crate) fn py_tuple_set_item(list: &Bound<PyTuple>, index: usize, value: Bound<PyAny>) {
    let index: Py_ssize_t = index.try_into().expect("size is too large");
    ffi!(PyTuple_SetItem(list.as_ptr(), index, value.into_ptr()));
}


#[inline]
fn py_object_call1_or_err(
    obj: *mut ffi::PyObject,
    args: *mut ffi::PyObject,
) -> PyResult<*mut ffi::PyObject> {
    from_ptr_or_err(ffi!(PyObject_CallObject(obj, args)))
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
