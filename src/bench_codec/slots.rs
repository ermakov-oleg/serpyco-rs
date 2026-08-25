//! Direct `__slots__` field access for the specialized codec (benchmark only).
//!
//! `setattr`/`getattr` on a `@dataclass(slots=True)` instance costs a
//! `_PyType_Lookup` (MRO cache probe keyed on the interned attribute name) plus
//! a descriptor call, per field. The layout of a slots class is fixed once the
//! class object exists, so the prototype resolves each field's byte offset at
//! construction time and reads/writes the instance memory directly.
//!
//! The offset comes from the `member_descriptor` in the class `__dict__`, and
//! every offset is re-verified against a real instance before use (see
//! [`verify_layout`]), so a layout surprise fails loudly at construction rather
//! than corrupting memory at run time.

use pyo3::exceptions::PyRuntimeError;
use pyo3::prelude::*;
use pyo3::types::PyType;
use pyo3::{ffi, PyResult};
use pyo3_ffi::Py_ssize_t;

/// `PyMemberDef` from CPython's `object.h`; stable across 3.10 … 3.14.
#[repr(C)]
struct PyMemberDef {
    name: *const std::os::raw::c_char,
    type_: std::os::raw::c_int,
    offset: Py_ssize_t,
    flags: std::os::raw::c_int,
    doc: *const std::os::raw::c_char,
}

/// `PyMemberDescrObject` from CPython's `descrobject.h`.
#[repr(C)]
struct PyMemberDescrObject {
    ob_base: ffi::PyObject,
    d_type: *mut ffi::PyTypeObject,
    d_name: *mut ffi::PyObject,
    d_qualname: *mut ffi::PyObject,
    d_member: *const PyMemberDef,
}

/// `Py_T_OBJECT_EX` — the member kind `__slots__` entries always use.
const T_OBJECT_EX: std::os::raw::c_int = 16;

/// Byte offset of slot `name` inside instances of `cls`.
pub(super) fn slot_offset(cls: &Bound<'_, PyType>, name: &str) -> PyResult<Py_ssize_t> {
    let descr = cls.getattr(name).map_err(|_| {
        PyRuntimeError::new_err(format!("{}: no attribute {name}", cls.name().unwrap()))
    })?;
    let type_name = descr.get_type().name()?.to_string();
    if type_name != "member_descriptor" {
        return Err(PyRuntimeError::new_err(format!(
            "{name} is a {type_name}, expected a __slots__ member_descriptor \
             (the prototype requires @dataclass(slots=True))"
        )));
    }
    // Safety: the object is a `member_descriptor`, so it has the layout above.
    let (offset, kind) = unsafe {
        let d = descr.as_ptr() as *const PyMemberDescrObject;
        let m = (*d).d_member;
        ((*m).offset, (*m).type_)
    };
    let basicsize = unsafe { (*cls.as_type_ptr()).tp_basicsize };
    if kind != T_OBJECT_EX
        || offset <= 0
        || offset + (std::mem::size_of::<usize>() as Py_ssize_t) > basicsize
    {
        return Err(PyRuntimeError::new_err(format!(
            "unexpected slot layout for {name}: kind={kind} offset={offset} basicsize={basicsize}"
        )));
    }
    Ok(offset)
}

/// Prove the discovered offsets before any direct write happens: build one
/// instance the ordinary way, stamp each slot through `setattr`, and check that
/// reading the raw offset gives the same object back.
pub(super) fn verify_layout(
    cls: &Bound<'_, PyType>,
    fields: &[(&str, Py_ssize_t)],
) -> PyResult<()> {
    let py = cls.py();
    let probe = super::alloc_instance(cls.as_type_ptr())?;
    let probe_any = unsafe { Bound::from_borrowed_ptr(py, probe.as_ptr()) };
    for (name, offset) in fields {
        let sentinel = pyo3::types::PyList::empty(py);
        probe_any
            .setattr(*name, &sentinel)
            .map_err(|e| PyRuntimeError::new_err(format!("probe setattr {name} failed: {e}")))?;
        let seen = unsafe { read_slot(probe.as_ptr(), *offset) };
        if seen != sentinel.as_ptr() {
            return Err(PyRuntimeError::new_err(format!(
                "slot offset {offset} for {name} does not match setattr result"
            )));
        }
    }
    Ok(())
}

/// Store an owned reference into a slot of a freshly allocated instance.
///
/// # Safety
/// `obj` must be an instance of the class the offset was resolved from, and the
/// slot must still hold its `tp_alloc` zero (each field is written once).
#[inline(always)]
pub(super) unsafe fn store_slot(
    obj: *mut ffi::PyObject,
    offset: Py_ssize_t,
    value: super::obj::Obj,
) {
    let p = obj.byte_offset(offset) as *mut *mut ffi::PyObject;
    debug_assert!((*p).is_null());
    *p = value.into_raw();
}

/// Borrowed read of a slot. Returns null for a slot never assigned.
///
/// # Safety
/// Same class invariant as [`store_slot`].
#[inline(always)]
pub(super) unsafe fn read_slot(obj: *mut ffi::PyObject, offset: Py_ssize_t) -> *mut ffi::PyObject {
    *(obj.byte_offset(offset) as *mut *mut ffi::PyObject)
}
