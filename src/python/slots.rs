//! Direct `__slots__` field access for entity encoders.
//!
//! Reading or writing a field of a `@dataclass(slots=True)` instance through
//! `getattr`/`setattr` costs a `_PyType_Lookup` on the MRO cache plus a
//! descriptor call, per field. The layout of a slots class is fixed once the
//! class object exists, so the offsets can be resolved when the serializer is
//! built and the instance memory addressed directly afterwards.
//!
//! [`resolve`] only reports a layout it has *proved*: every offset is written
//! and read back through the ordinary attribute protocol on a probe instance
//! before the encoder is allowed to use it. Anything unusual about the class —
//! an instance `__dict__`, a non-slot field, a custom `__setattr__`/`__getattr__`
//! — disables the whole optimization for that entity and the descriptor path
//! stays in use.

use pyo3::prelude::*;
use pyo3::types::{PyString, PyType};
use pyo3::{ffi, PyResult};
use pyo3_ffi::Py_ssize_t;

use super::py::{create_instance, generic_set_attr};

/// `PyMemberDef` from CPython's `object.h`; the layout is stable across
/// 3.10 … 3.14 and is what a `__slots__` descriptor points at.
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

/// `Py_T_OBJECT_EX`, the member kind `__slots__` entries always use.
const T_OBJECT_EX: std::os::raw::c_int = 16;
/// `Py_READONLY`.
const READONLY: std::os::raw::c_int = 1;

/// Byte offsets of an entity's fields, in the order they were passed to
/// [`resolve`]. Present only when every one of them was verified.
pub(crate) type SlotOffsets = Box<[Py_ssize_t]>;

/// Resolve and verify the slot offsets of `names` on `cls`.
///
/// `is_frozen` mirrors what the entity encoder already does: a frozen
/// dataclass's `__setattr__` is bypassed via `PyObject_GenericSetAttr` anyway,
/// so a direct store changes nothing observable. For anything else a custom
/// `__setattr__` must keep being called, so the optimization is refused.
///
/// Returns `None` — not an error — whenever the class is not a plain slots
/// layout; the caller then keeps using the descriptor path.
pub(crate) fn resolve(
    cls: &Bound<'_, PyType>,
    names: &[&Py<PyString>],
    is_frozen: bool,
) -> Option<SlotOffsets> {
    let py = cls.py();
    let tp = cls.as_type_ptr();
    // Safety: `tp` is a live type object for as long as `cls` is bound.
    let (basicsize, dictoffset, setattro, getattro) = unsafe {
        (
            (*tp).tp_basicsize,
            (*tp).tp_dictoffset,
            (*tp).tp_setattro,
            (*tp).tp_getattro,
        )
    };

    // `tp_flags` is atomic on free-threaded builds, so go through the accessor.
    if unsafe { ffi::PyType_HasFeature(tp, ffi::Py_TPFLAGS_HEAPTYPE) } == 0 {
        return None; // static type: not a dataclass
    }
    if dictoffset != 0 {
        // An instance `__dict__` means attributes can also live outside the
        // slots; stay on the descriptor path rather than reason about it.
        return None;
    }
    // Both sides are CPython's own exported C functions, so comparing their
    // addresses is meaningful (the lint is about Rust functions, which the
    // linker may merge or duplicate across codegen units).
    let generic_get = getattro.is_some_and(|f| {
        std::ptr::fn_addr_eq(f, ffi::PyObject_GenericGetAttr as ffi::getattrofunc)
    });
    let generic_set = setattro.is_some_and(|f| {
        std::ptr::fn_addr_eq(f, ffi::PyObject_GenericSetAttr as ffi::setattrofunc)
    });
    if !generic_get || !(generic_set || is_frozen) {
        return None;
    }

    let mut offsets = Vec::with_capacity(names.len());
    for name in names {
        offsets.push(member_offset(cls, name.bind(py), basicsize)?);
    }
    let offsets: SlotOffsets = offsets.into_boxed_slice();
    verify(cls, names, &offsets).ok()?.then_some(offsets)
}

/// Offset of `name`'s `__slots__` member, or `None` if it is not one.
fn member_offset(
    cls: &Bound<'_, PyType>,
    name: &Bound<'_, PyString>,
    basicsize: Py_ssize_t,
) -> Option<Py_ssize_t> {
    let descr = cls.getattr(name).ok()?;
    // Exact type identity against CPython's `PyMemberDescr_Type`: this is what
    // makes reading `d_member` below sound, so it must not be a name check.
    if !std::ptr::eq(
        unsafe { ffi::Py_TYPE(descr.as_ptr()) },
        &raw mut pyo3_ffi::PyMemberDescr_Type,
    ) {
        return None;
    }
    // Safety: the object is a `member_descriptor`, so it has this layout.
    let (offset, kind, flags) = unsafe {
        let m = (*(descr.as_ptr() as *const PyMemberDescrObject)).d_member;
        ((*m).offset, (*m).type_, (*m).flags)
    };
    let fits =
        offset > 0 && offset.checked_add(std::mem::size_of::<usize>() as Py_ssize_t)? <= basicsize;
    (kind == T_OBJECT_EX && flags & READONLY == 0 && fits).then_some(offset)
}

/// Prove the offsets before anything writes through them: stamp each one on a
/// throwaway instance and read it back through the ordinary attribute protocol.
fn verify(
    cls: &Bound<'_, PyType>,
    names: &[&Py<PyString>],
    offsets: &[Py_ssize_t],
) -> PyResult<bool> {
    let py = cls.py();
    let probe = create_instance(cls)?;
    for (name, &offset) in names.iter().zip(offsets) {
        let name = name.bind(py);
        // A sentinel with a stable identity that cannot be interned or cached.
        let sentinel = pyo3::types::PyList::empty(py);
        generic_set_attr(&probe, name.as_ptr(), sentinel.clone().into_any())?;
        // Safety: `offset` is within `tp_basicsize` and `probe` is an instance
        // of `cls`; the read is a borrowed pointer, possibly null.
        let seen = unsafe { read_slot_raw(probe.as_ptr(), offset) };
        if seen != sentinel.as_ptr() {
            return Ok(false);
        }
    }
    Ok(true)
}

/// Store an owned reference into a slot, releasing whatever was there.
///
/// # Safety
/// `obj` must be an instance of the class `offset` was resolved from.
#[inline(always)]
pub(crate) unsafe fn store_slot(obj: *mut ffi::PyObject, offset: Py_ssize_t, value: Bound<PyAny>) {
    let place = obj.byte_offset(offset) as *mut *mut ffi::PyObject;
    let previous = *place;
    *place = value.into_ptr();
    ffi::Py_XDECREF(previous);
}

/// Read a slot, or `None` if the field was never assigned.
///
/// Only compiled for GIL builds. Writing goes into an instance this thread just
/// allocated and has not published yet, but *reading* races with any other
/// thread mutating the same object: on a free-threaded build `PyMember_GetOne`
/// pairs an atomic load with a try-incref to make that safe, and a plain load
/// followed by an incref cannot. There the descriptor path stays in use.
///
/// # Safety
/// Same class invariant as [`store_slot`].
#[cfg(not(Py_GIL_DISABLED))]
#[inline(always)]
pub(crate) unsafe fn read_slot<'py>(
    py: Python<'py>,
    obj: *mut ffi::PyObject,
    offset: Py_ssize_t,
) -> Option<Bound<'py, PyAny>> {
    let value = read_slot_raw(obj, offset);
    (!value.is_null()).then(|| Bound::from_borrowed_ptr(py, value))
}

/// # Safety
/// Same class invariant as [`store_slot`].
#[inline(always)]
unsafe fn read_slot_raw(obj: *mut ffi::PyObject, offset: Py_ssize_t) -> *mut ffi::PyObject {
    *(obj.byte_offset(offset) as *mut *mut ffi::PyObject)
}
