use std::slice::Iter;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Context {
    pub path: JSONPointer,
    // pub try_cast: bool,
}

impl Context {
    pub fn new() -> Self {
        Context {
            path: JSONPointer::default(),
        }
    }

    pub fn push(self, chunk: impl Into<PathChunk>) -> Self {
        let path = self.path.clone_with(chunk);
        Context { path }
    }

    pub fn extend(self, chunks: &[PathChunk]) -> Self {
        let path = self.path.extend_with(chunks);
        Context { path }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PathChunk {
    /// Property name within a JSON object.
    Property(Box<str>),
    /// Index within a JSON array.
    Index(usize),
    /// JSON Schema keyword.
    Keyword(&'static str),
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
/// JSON Pointer as a wrapper around individual path components.
pub struct JSONPointer(Vec<PathChunk>);

impl JSONPointer {
    /// JSON pointer as a vector of strings. Each component is casted to `String`. Consumes `JSONPointer`.
    #[must_use]
    pub fn into_vec(self) -> Vec<String> {
        self.0
            .into_iter()
            .map(|item| match item {
                PathChunk::Property(value) => value.into_string(),
                PathChunk::Index(idx) => idx.to_string(),
                PathChunk::Keyword(keyword) => keyword.to_string(),
            })
            .collect()
    }

    // /// Return an iterator over the underlying vector of path components.
    pub fn iter(&self) -> Iter<'_, PathChunk> {
        self.0.iter()
    }
    /// Take the last pointer chunk.
    #[must_use]
    #[inline]
    pub fn last(&self) -> Option<&PathChunk> {
        self.0.last()
    }

    pub(crate) fn clone_with(&self, chunk: impl Into<PathChunk>) -> Self {
        let mut new = self.clone();
        new.0.push(chunk.into());
        new
    }

    pub(crate) fn extend_with(&self, chunks: &[PathChunk]) -> Self {
        let mut new = self.clone();
        new.0.extend_from_slice(chunks);
        new
    }

    pub(crate) fn as_slice(&self) -> &[PathChunk] {
        &self.0
    }
}


#[derive(Debug, Clone)]
pub struct InstancePath<'a> {
    pub(crate) chunk: Option<PathChunk>,
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
    pub(crate) fn push(&'a self, chunk: impl Into<PathChunk>) -> Self {
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

impl From<String> for PathChunk {
    #[inline]
    fn from(value: String) -> Self {
        PathChunk::Property(value.into_boxed_str())
    }
}
impl From<&'static str> for PathChunk {
    #[inline]
    fn from(value: &'static str) -> Self {
        PathChunk::Keyword(value)
    }
}
impl From<usize> for PathChunk {
    #[inline]
    fn from(value: usize) -> Self {
        PathChunk::Index(value)
    }
}


impl<'a> From<&'a InstancePath<'a>> for JSONPointer {
    #[inline]
    fn from(path: &'a InstancePath<'a>) -> Self {
        JSONPointer(path.to_vec())
    }
}

impl From<InstancePath<'_>> for JSONPointer {
    #[inline]
    fn from(path: InstancePath<'_>) -> Self {
        JSONPointer(path.to_vec())
    }
}
