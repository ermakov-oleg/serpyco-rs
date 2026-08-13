use std::os::raw::c_int;

use pyo3::exceptions::PyOverflowError;
use pyo3::prelude::*;
use pyo3::types::{PyDict, PyList, PyString, PyTuple, PyType};
use pyo3::{ffi, PyErr, PyResult, Python};
use pyo3_ffi::Py_ssize_t;

use super::macros::ffi;

#[cold]
#[inline(never)]
fn err_size_overflow() -> PyErr {
    PyOverflowError::new_err("collection size exceeds Py_ssize_t::MAX")
}

#[cold]
#[inline(never)]
fn err_alloc_failed(py: Python) -> PyErr {
    PyErr::fetch(py)
}

#[inline(always)]
fn cast_size(size: usize) -> Result<Py_ssize_t, ()> {
    if size <= Py_ssize_t::MAX as usize {
        Ok(size as Py_ssize_t)
    } else {
        Err(())
    }
}

#[inline(always)]
pub(crate) fn create_py_list(py: Python, size: usize) -> PyResult<Bound<PyList>> {
    let size = cast_size(size).map_err(|_| err_size_overflow())?;
    let ptr = unsafe { ffi::PyList_New(size) };
    if ptr.is_null() {
        return Err(err_alloc_failed(py));
    }
    Ok(unsafe { Bound::from_owned_ptr(py, ptr).cast_into_unchecked() })
}

#[inline(always)]
pub(crate) fn py_list_get_item<'a>(
    list: &'a Bound<'a, PyList>,
    index: usize,
) -> PyResult<Bound<'a, PyAny>> {
    #[cfg(any(Py_LIMITED_API, PyPy))]
    let item = list.get_item(index)?;
    // Safety: caller obtained `index` from iterating over the same list it
    // is now indexing into, so `index < list.len() <= Py_ssize_t::MAX`.
    #[cfg(not(any(Py_LIMITED_API, PyPy)))]
    let item = unsafe { list.get_item_unchecked(index) };
    Ok(item)
}

#[inline]
pub(crate) fn py_list_set_item(list: &Bound<PyList>, index: usize, value: Bound<PyAny>) {
    // Safety: caller invariant — `index` always comes from `0..list.len()`,
    // and `list.len()` was validated via `size_to_py_ssize_t` at creation.
    debug_assert!(index <= Py_ssize_t::MAX as usize);
    let index = index as Py_ssize_t;
    #[cfg(not(Py_LIMITED_API))]
    ffi!(PyList_SET_ITEM(list.as_ptr(), index, value.into_ptr()));
    #[cfg(Py_LIMITED_API)]
    ffi!(PyList_SetItem(list.as_ptr(), index, value.into_ptr()));
}

// Private CPython API: the binding was removed from pyo3-ffi 0.28+, but the
// symbol is still exported by all supported CPython versions (incl. 3.14t).
// On Windows this needs the import library, which pyo3 0.29+ no longer links
// (raw-dylib) — build.rs restores it.
extern "C" {
    fn _PyDict_NewPresized(minused: Py_ssize_t) -> *mut ffi::PyObject;
}

#[inline(always)]
pub(crate) fn create_py_dict_known_size(py: Python, size: usize) -> PyResult<Bound<PyDict>> {
    let size = cast_size(size).map_err(|_| err_size_overflow())?;
    let ptr = unsafe { _PyDict_NewPresized(size) };
    if ptr.is_null() {
        return Err(err_alloc_failed(py));
    }
    Ok(unsafe { Bound::from_owned_ptr(py, ptr).cast_into_unchecked() })
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

/// Set attribute by calling the object's type `tp_setattro` slot directly.
///
/// `PyObject_SetAttr` (which `Bound::setattr` ultimately calls) interns the
/// attribute name on every call via `PyUnicode_InternInPlace`. The attribute
/// names we store come from `Py<PyString>` allocated once at serializer
/// build-time and are already interned, so that work is pure overhead.
/// Calling `tp_setattro` directly skips the intern step.
#[inline]
pub(crate) fn set_attr_unchecked(
    obj: &Bound<PyAny>,
    name: *mut ffi::PyObject,
    value: Bound<PyAny>,
) -> PyResult<()> {
    let result = unsafe {
        let tp = ffi::Py_TYPE(obj.as_ptr());
        match (*tp).tp_setattro {
            Some(setattro) => setattro(obj.as_ptr(), name, value.as_ptr()),
            None => ffi::PyObject_SetAttr(obj.as_ptr(), name, value.as_ptr()),
        }
    };
    error_on_minusone(result)
}

/// Allocate an instance of `cls` via its `tp_alloc` slot, without calling
/// `__new__`/`__init__`.
///
/// Equivalent to the `object.__new__(cls)` call it replaces: for classes defined
/// in Python `object.__new__` reduces to `type->tp_alloc(type, 0)` plus call
/// machinery (argument tuple, `PyObject_Call`, excess-args checks that never fire
/// here), and `__init__`/`__post_init__` were never invoked on this path either.
/// A custom `tp_alloc` from a C-level metaclass is honored, matching what
/// `object.__new__` would do. `PyType_GenericAlloc` returns zero-initialized
/// memory and handles GC tracking itself.
///
/// Caller invariant: every dataclass field must be assigned via setattr before
/// the instance is handed to user code.
#[inline]
pub(crate) fn create_instance<'py>(cls: &Bound<'py, PyType>) -> PyResult<Bound<'py, PyAny>> {
    let tp = cls.as_type_ptr();
    let ptr = unsafe {
        let alloc = (*tp).tp_alloc.unwrap_or(ffi::PyType_GenericAlloc);
        alloc(tp, 0)
    };
    if ptr.is_null() {
        return Err(err_alloc_failed(cls.py()));
    }
    Ok(unsafe { Bound::from_owned_ptr(cls.py(), ptr) })
}

/// Set attribute via `PyObject_GenericSetAttr`, bypassing the type's `tp_setattro`.
///
/// Used for frozen dataclasses: their `__setattr__` raises `FrozenInstanceError`,
/// so fields must be written the way `object.__setattr__` would. Calling the C
/// function directly skips the bound-method call machinery (argument tuple,
/// wrapper descriptor) that `object.__setattr__(obj, name, value)` pays per field.
#[inline]
pub(crate) fn generic_set_attr(
    obj: &Bound<PyAny>,
    name: *mut ffi::PyObject,
    value: Bound<PyAny>,
) -> PyResult<()> {
    let result = unsafe { ffi::PyObject_GenericSetAttr(obj.as_ptr(), name, value.as_ptr()) };
    error_on_minusone(result)
}

#[inline(always)]
pub(crate) fn create_py_tuple(py: Python, size: usize) -> PyResult<Bound<PyTuple>> {
    let size = cast_size(size).map_err(|_| err_size_overflow())?;
    let ptr = unsafe { ffi::PyTuple_New(size) };
    if ptr.is_null() {
        return Err(err_alloc_failed(py));
    }
    Ok(unsafe { Bound::from_owned_ptr(py, ptr).cast_into_unchecked() })
}

#[inline]
pub(crate) fn py_tuple_set_item(list: &Bound<PyTuple>, index: usize, value: Bound<PyAny>) {
    // Safety: caller invariant — `index` always comes from `0..tuple.len()`,
    // and `tuple.len()` was validated via `size_to_py_ssize_t` at creation.
    debug_assert!(index <= Py_ssize_t::MAX as usize);
    let index = index as Py_ssize_t;
    ffi!(PyTuple_SetItem(list.as_ptr(), index, value.into_ptr()));
}

#[inline]
pub(crate) fn error_on_minusone(result: c_int) -> PyResult<()> {
    if result != -1 {
        Ok(())
    } else {
        Err(Python::attach(PyErr::fetch))
    }
}

/// Build a `str` from bytes already known to be pure ASCII, skipping CPython's
/// maxchar re-scan inside `PyUnicode_FromStringAndSize`: allocate the compact
/// ASCII representation directly and memcpy the payload.
///
/// # Safety
/// `bytes` must be ASCII-only (every byte < 0x80).
#[inline(always)]
pub(crate) unsafe fn pystring_ascii_new<'py>(
    py: Python<'py>,
    bytes: &[u8],
) -> PyResult<Bound<'py, PyString>> {
    let obj = ffi::PyUnicode_New(bytes.len() as Py_ssize_t, 127);
    if obj.is_null() {
        return Err(err_alloc_failed(py));
    }
    debug_assert!(bytes.is_ascii());
    std::ptr::copy_nonoverlapping(bytes.as_ptr(), ffi::PyUnicode_1BYTE_DATA(obj), bytes.len());
    Ok(Bound::from_owned_ptr(py, obj).cast_into_unchecked())
}

/// `PyString::new` with an ASCII fast path: one cheap vectorized `is_ascii`
/// scan, then the direct compact-ASCII constructor; non-ASCII falls back to
/// CPython's UTF-8 decoder. Use on hot paths that materialize parsed strings.
#[inline(always)]
pub(crate) fn create_py_string<'py>(py: Python<'py>, s: &str) -> PyResult<Bound<'py, PyString>> {
    let bytes = s.as_bytes();
    if bytes.is_ascii() {
        unsafe { pystring_ascii_new(py, bytes) }
    } else {
        Ok(PyString::new(py, s))
    }
}
