use super::escape;

/// Separators are trailing (`item_end` after each element; the closer drops the
/// leftover comma), so the per-element path is an unconditional push with no
/// open-container stack. Call contract: `map_key`/`map_key_encoded` before each
/// map value, `item_end` after each map value and array element.
#[derive(Debug)]
pub(crate) struct JsonWriter {
    buf: Vec<u8>,
}

/// Saved position for a speculative write that may be rolled back (union member
/// probing, omit_none null-skip) — the buffer length alone is enough to truncate back to it.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Checkpoint {
    buf_len: usize,
}

/// Render `"key":` once, at encoder-construction time. Escaping still runs:
/// a JSON key can come from an `Alias`, not just a Python identifier.
pub(crate) fn encode_map_key(key: &str) -> Box<[u8]> {
    let mut buf = Vec::with_capacity(key.len() + 3);
    buf.push(b'"');
    escape::escape_into(&mut buf, key);
    buf.extend_from_slice(b"\":");
    buf.into_boxed_slice()
}

impl JsonWriter {
    pub(crate) fn with_capacity(capacity: usize) -> Self {
        JsonWriter {
            buf: Vec::with_capacity(capacity),
        }
    }

    #[inline]
    pub(crate) fn as_bytes(&self) -> &[u8] {
        &self.buf
    }

    #[inline]
    pub(crate) fn item_end(&mut self) {
        self.buf.push(b',');
    }

    /// Only this writer's own trailing separator can be the last byte — a nested
    /// value always ends in `"`, a digit, `e`, `l` (null/true), `}` or `]`.
    #[inline]
    fn close(&mut self, terminator: u8) {
        if self.buf.last() == Some(&b',') {
            self.buf.pop();
        }
        self.buf.push(terminator);
    }

    #[inline]
    pub(crate) fn write_null(&mut self) {
        self.buf.extend_from_slice(b"null");
    }

    #[inline]
    pub(crate) fn write_bool(&mut self, v: bool) {
        self.buf
            .extend_from_slice(if v { b"true" } else { b"false" });
    }

    #[inline]
    pub(crate) fn write_i64(&mut self, v: i64) {
        let mut b = itoa::Buffer::new();
        self.buf.extend_from_slice(b.format(v).as_bytes());
    }

    #[inline]
    pub(crate) fn write_raw_number(&mut self, v: &str) {
        self.buf.extend_from_slice(v.as_bytes());
    }

    #[inline(always)]
    pub(crate) fn position(&self) -> usize {
        self.buf.len()
    }

    #[inline(always)]
    pub(crate) fn checkpoint(&self) -> Checkpoint {
        Checkpoint {
            buf_len: self.buf.len(),
        }
    }

    #[inline(always)]
    pub(crate) fn rollback(&mut self, cp: Checkpoint) {
        self.buf.truncate(cp.buf_len);
    }

    #[inline(always)]
    pub(crate) fn tail_is_null(&self, from: usize) -> bool {
        self.buf[from..] == *b"null"
    }

    #[inline]
    pub(crate) fn write_f64(&mut self, v: f64) -> Result<(), &'static str> {
        if !v.is_finite() {
            return Err("NaN and Infinity are not allowed in JSON");
        }
        let mut b = ryu::Buffer::new();
        self.buf.extend_from_slice(b.format_finite(v).as_bytes());
        Ok(())
    }

    #[inline]
    pub(crate) fn write_str(&mut self, v: &str) {
        // One growth check for the whole string instead of one per push/extend.
        self.buf.reserve(v.len() + 2);
        self.buf.push(b'"');
        escape::escape_into(&mut self.buf, v);
        self.buf.push(b'"');
    }

    #[inline]
    pub(crate) fn begin_map(&mut self) {
        self.buf.push(b'{');
    }

    #[inline]
    pub(crate) fn map_key(&mut self, key: &str) {
        self.write_str(key);
        self.buf.push(b':');
    }

    /// Append a key already rendered by [`encode_map_key`] — quotes, escaping and `:` included.
    #[inline]
    pub(crate) fn map_key_encoded(&mut self, encoded: &[u8]) {
        self.buf.extend_from_slice(encoded);
    }

    #[inline]
    pub(crate) fn end_map(&mut self) {
        self.close(b'}');
    }

    #[inline]
    pub(crate) fn begin_array(&mut self) {
        self.buf.push(b'[');
    }

    #[inline]
    pub(crate) fn end_array(&mut self) {
        self.close(b']');
    }
}
