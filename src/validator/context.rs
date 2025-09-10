use pyo3::{Bound, PyAny};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Context {
    pub try_cast_from_string: bool,
}

impl Context {
    pub fn new(try_cast_from_string: bool) -> Self {
        Context {
            try_cast_from_string,
        }
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
