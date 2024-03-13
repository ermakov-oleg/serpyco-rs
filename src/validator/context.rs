use pyo3::{Bound, PyAny};
use pyo3::types::PyString;
use crate::validator::Value;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Context {
    // pub try_cast: bool,
}

impl Context {
    pub fn new() -> Self {
        Context {}
    }
}

#[derive(Clone, Debug)]
pub enum PathChunk<'a> {
    /// Property name within a JSON object.
    Property(Box<str>),
    /// Index within a JSON array.
    Index(isize),  // todo: DROP
    Index2(usize),
    /// Python value
    PropertyPyValue(&'a Value),  // todo: DROP
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

    pub(crate) fn to_vec(&'a self) -> Vec<PathChunk> {
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

impl<'a> From<String> for PathChunk<'a> {
    #[inline]
    fn from(value: String) -> Self {
        PathChunk::Property(value.into_boxed_str())
    }
}


impl<'a> From<&str> for PathChunk<'a> {
    #[inline]
    fn from(value: &str) -> Self {
        PathChunk::Property(value.into())
    }
}

impl<'a> From<isize> for PathChunk<'a> {
    #[inline]
    fn from(value: isize) -> Self {
        PathChunk::Index(value)
    }
}


impl<'a> From<usize> for PathChunk<'a> {
    #[inline]
    fn from(value: usize) -> Self {
        PathChunk::Index2(value)
    }
}


impl<'a> From<&'a Value> for PathChunk<'a> {
    #[inline]
    fn from(value: &'a Value) -> Self {
        PathChunk::PropertyPyValue(value)
    }
}

impl<'a> From<&'a Bound<'a, PyAny>> for PathChunk<'a> {
    #[inline]
    fn from(value: &'a Bound<'a, PyAny>) -> Self {
        PathChunk::PropertyValue(value)
    }
}
