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
//!
//! A class stays mutable after the serializer is built, so "proved once" is not
//! enough: replacing a field with a `property`, or installing a `__setattr__`,
//! would leave the offsets pointing at storage the attribute protocol no longer
//! uses. [`resolve`] therefore also records the class's `tp_version_tag`, and
//! every access re-checks it — CPython bumps it on any change to the type or
//! its bases, the same signal its own inline caches watch.
//!
//! A stale tag means "re-check", not "give up for good". Perfectly ordinary code
//! mutates the class once and never again — the first `pickle` or `copy.deepcopy`
//! of a slots instance makes `copyreg` cache `__slotnames__` in the class dict —
//! and a serializer that lost the fast path there would never get it back. So a
//! mismatch re-runs [`resolve`]: if the layout still holds, the new version is
//! adopted and the next call is fast again; if it does not, the entity drops to
//! the descriptor path permanently.
//!
//! The whole thing is off on free-threaded builds. A direct store is only sound
//! while nothing else can reach the instance, and that does not hold: user code
//! runs mid-load (a `default_factory`, a custom encoder) and can pull the
//! half-built object out of `gc.get_objects()`. Once it has escaped, a plain
//! store races with a descriptor read in another thread, which CPython
//! synchronizes with a critical section and an atomic store that this cannot
//! reproduce from the outside.

use std::os::raw::c_uint;

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

/// A verified slot layout: one byte offset per field, in the order the names
/// were passed to [`resolve`], plus the class version it was verified against.
pub(crate) struct SlotLayout {
    pub(crate) offsets: Box<[Py_ssize_t]>,
    /// `tp_version_tag` at verification time; [`version_of`] must still match
    /// it before any offset is used.
    pub(crate) version: c_uint,
}

/// The class's current `tp_version_tag`. CPython invalidates it (to 0, then a
/// fresh value on the next lookup) whenever the type or one of its bases is
/// modified, so an unequal value means the verified layout is stale.
///
/// This is the same invariant CPython's own inline caches rest on: a C
/// extension that edits a type dict without `PyType_Modified` would defeat both.
/// Once the interpreter runs out of version numbers it stops handing them out,
/// leaving 0 here — which reads as "stale" and simply retires the fast path.
///
/// # Safety
/// `tp` must be a live type object.
#[inline(always)]
pub(crate) unsafe fn version_of(tp: *mut ffi::PyTypeObject) -> c_uint {
    (*tp).tp_version_tag
}

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
) -> Option<SlotLayout> {
    // See the module docs: sound only while the instance cannot be reached from
    // another thread, which free-threaded builds cannot guarantee.
    if cfg!(Py_GIL_DISABLED) {
        return None;
    }
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
    let offsets = offsets.into_boxed_slice();
    if !verify(cls, names, &offsets).ok()? {
        return None;
    }
    // Read the tag last: `member_offset` and `verify` both go through
    // `_PyType_Lookup`, which is what assigns one. A type that still has no
    // version cannot be guarded, so it does not get the fast path.
    let version = unsafe { version_of(tp) };
    (version != 0).then_some(SlotLayout { offsets, version })
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
/// Never reached on free-threaded builds: [`resolve`] hands out no layout there.
///
/// # Safety
/// Same class invariant as [`store_slot`].
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
