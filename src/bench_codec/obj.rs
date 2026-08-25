//! Owned strong reference used across the specialized codec (benchmark only).
//!
//! `Bound<'py, PyAny>` would do the same job, but the load path never needs the
//! GIL token it carries: every value is produced and immediately stored into a
//! slot. `Obj` is a bare pointer with a `Drop`, so the happy path compiles to a
//! move and only error unwinding pays a decref.

use std::ptr::NonNull;

use pyo3::ffi;

/// `NonNull` rather than a raw pointer so `Option<Obj>` — and therefore [`Slot`]
/// — stays one machine word.
#[repr(transparent)]
pub(super) struct Obj(NonNull<ffi::PyObject>);

impl Obj {
    /// # Safety
    /// `p` must be a non-null owned (strong) reference.
    #[inline(always)]
    pub(super) unsafe fn from_owned(p: *mut ffi::PyObject) -> Self {
        debug_assert!(!p.is_null());
        Obj(NonNull::new_unchecked(p))
    }

    /// # Safety
    /// `p` must be a non-null borrowed reference that outlives the incref.
    #[inline(always)]
    pub(super) unsafe fn new_ref(p: *mut ffi::PyObject) -> Self {
        debug_assert!(!p.is_null());
        ffi::Py_INCREF(p);
        Obj(NonNull::new_unchecked(p))
    }

    #[inline(always)]
    pub(super) fn none() -> Self {
        unsafe { Obj::new_ref(ffi::Py_None()) }
    }

    #[inline(always)]
    pub(super) fn bool(v: bool) -> Self {
        unsafe { Obj::new_ref(if v { ffi::Py_True() } else { ffi::Py_False() }) }
    }

    #[inline(always)]
    pub(super) fn as_ptr(&self) -> *mut ffi::PyObject {
        self.0.as_ptr()
    }

    #[inline(always)]
    pub(super) fn into_raw(self) -> *mut ffi::PyObject {
        let p = self.0.as_ptr();
        std::mem::forget(self);
        p
    }
}

impl Drop for Obj {
    #[inline(always)]
    fn drop(&mut self) {
        unsafe { ffi::Py_DECREF(self.0.as_ptr()) }
    }
}

/// A field value that may still be unset while the object is being read.
///
/// Replaces the real codec's `SeenSet` bitmask: "not seen" is just `None`, and
/// the local holds the parsed value until the instance is filled in one pass.
pub(super) struct Slot(Option<Obj>);

impl Slot {
    #[inline(always)]
    pub(super) fn empty() -> Self {
        Slot(None)
    }

    #[inline(always)]
    pub(super) fn set(&mut self, v: Obj) {
        self.0 = Some(v); // a duplicate key drops the earlier value
    }

    #[inline(always)]
    pub(super) fn take(self) -> Option<Obj> {
        self.0
    }
}
