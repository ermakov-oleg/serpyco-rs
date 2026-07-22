use std::cell::Cell;

use pyo3::exceptions::PyRecursionError;
use pyo3::{Bound, PyAny};

use crate::serde_error::{SerdeError, SerdeResult};

#[derive(Debug)]
pub struct Context {
    pub try_cast_from_string: bool,
    /// Depth of the currently active encoder recursion. Updated via
    /// `enter_depth` / `DepthGuard::drop`. Prevents stack overflow on cyclic
    /// or pathologically deep inputs by surfacing `PyRecursionError` once the
    /// counter crosses `max_depth`.
    depth: Cell<usize>,
    max_depth: usize,
}

impl Context {
    pub fn new(try_cast_from_string: bool, max_depth: usize) -> Self {
        Context {
            try_cast_from_string,
            depth: Cell::new(0),
            max_depth,
        }
    }

    /// Increment the recursion counter and return a RAII guard that decrements
    /// it on drop. Returns an error when `max_depth` is exceeded. The guard
    /// must be bound to a local (e.g. `let _guard = ctx.enter_depth()?;`) so
    /// that its lifetime spans the recursive call.
    #[inline]
    pub fn enter_depth(&self) -> SerdeResult<DepthGuard<'_>> {
        let next = self.depth.get() + 1;
        if next > self.max_depth {
            return Err(SerdeError::Py(PyRecursionError::new_err(format!(
                "maximum recursion depth exceeded ({}); likely a cyclic graph or excessively deep structure",
                self.max_depth
            ))));
        }
        self.depth.set(next);
        Ok(DepthGuard { ctx: self })
    }
}

pub struct DepthGuard<'a> {
    ctx: &'a Context,
}

impl Drop for DepthGuard<'_> {
    #[inline]
    fn drop(&mut self) {
        self.ctx.depth.set(self.ctx.depth.get() - 1);
    }
}

#[derive(Clone, Debug)]
pub enum PathChunk<'a> {
    /// Property name within a JSON object.
    Property(Box<str>),
    /// Index within a JSON array.
    Index(usize),
    /// Python value
    PropertyValue(&'a Bound<'a, PyAny>),
}

#[derive(Debug, Clone)]
pub struct InstancePath<'a> {
    pub(crate) chunk: Option<PathChunk<'a>>,
    pub(crate) parent: Option<&'a InstancePath<'a>>,
}

impl<'a> InstancePath<'a> {
    pub(crate) const fn new() -> Self {
        InstancePath {
            chunk: None,
            parent: None,
        }
    }

    #[inline]
    pub(crate) fn push(&'a self, chunk: impl Into<PathChunk<'a>>) -> Self {
        InstancePath {
            chunk: Some(chunk.into()),
            parent: Some(self),
        }
    }

    pub(crate) fn to_vec(&self) -> Vec<PathChunk<'_>> {
        // The path capacity should be the average depth so we avoid extra allocations
        let mut result = Vec::with_capacity(6);
        let mut current = self;
        if let Some(chunk) = &current.chunk {
            result.push(chunk.clone())
        }
        while let Some(next) = current.parent {
            current = next;
            if let Some(chunk) = &current.chunk {
                result.push(chunk.clone())
            }
        }
        result.reverse();
        result
    }
}

impl From<String> for PathChunk<'_> {
    #[inline]
    fn from(value: String) -> Self {
        PathChunk::Property(value.into_boxed_str())
    }
}

impl From<&str> for PathChunk<'_> {
    #[inline]
    fn from(value: &str) -> Self {
        PathChunk::Property(value.into())
    }
}

impl From<usize> for PathChunk<'_> {
    #[inline]
    fn from(value: usize) -> Self {
        PathChunk::Index(value)
    }
}

impl<'a> From<&'a Bound<'a, PyAny>> for PathChunk<'a> {
    #[inline]
    fn from(value: &'a Bound<'a, PyAny>) -> Self {
        PathChunk::PropertyValue(value)
    }
}
