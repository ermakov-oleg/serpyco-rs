/// Streaming JSON writer: writes into Vec<u8> and manages commas itself.
/// Call contract: map_key before each map value, array_item before each array element.
#[derive(Debug)]
pub(crate) struct JsonWriter {
    buf: Vec<u8>,
    /// true = the current container already has an element (comma needed).
    has_item: Vec<bool>,
}

/// A saved writer position for a speculative write that may be rolled back
/// (union member probing, omit_none null-skip). Captures the output length and
/// the current container's comma state so `rollback` fully undoes a partial or
/// unwanted value without a separate probe buffer + splice.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Checkpoint {
    buf_len: usize,
    has_item_len: usize,
    top_had_item: bool,
}

impl JsonWriter {
    pub(crate) fn new() -> Self {
        JsonWriter {
            buf: Vec::with_capacity(1024),
            has_item: Vec::with_capacity(8),
        }
    }

    #[inline]
    pub(crate) fn as_bytes(&self) -> &[u8] {
        &self.buf
    }

    #[inline]
    fn comma(&mut self) {
        if let Some(last) = self.has_item.last_mut() {
            if *last {
                self.buf.push(b',');
            }
            *last = true;
        }
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

    /// Current output length — the start position of the next value written.
    #[inline(always)]
    pub(crate) fn position(&self) -> usize {
        self.buf.len()
    }

    /// Snapshot the writer state so a speculative value can be rolled back.
    #[inline(always)]
    pub(crate) fn checkpoint(&self) -> Checkpoint {
        Checkpoint {
            buf_len: self.buf.len(),
            has_item_len: self.has_item.len(),
            top_had_item: self.has_item.last().copied().unwrap_or(false),
        }
    }

    /// Undo everything written since `cp`: truncate the buffer and restore the
    /// container-nesting/comma state (a failed union member may have left open
    /// containers; an unwanted omit_none value plus its key are dropped).
    #[inline(always)]
    pub(crate) fn rollback(&mut self, cp: Checkpoint) {
        self.buf.truncate(cp.buf_len);
        self.has_item.truncate(cp.has_item_len);
        if let Some(top) = self.has_item.last_mut() {
            *top = cp.top_had_item;
        }
    }

    /// Whether the value written since `from` is exactly the JSON null literal.
    /// Lets omit_none decide null-ness on the value already in the buffer, no probe.
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
        self.buf.push(b'"');
        self.escape_into(v);
        self.buf.push(b'"');
    }

    /// Escaping: ", \, control (<0x20). Non-ASCII is written as-is (UTF-8).
    fn escape_into(&mut self, v: &str) {
        let bytes = v.as_bytes();
        let mut start = 0;
        for (i, &b) in bytes.iter().enumerate() {
            let esc: Option<&[u8]> = match b {
                b'"' => Some(b"\\\""),
                b'\\' => Some(b"\\\\"),
                b'\n' => Some(b"\\n"),
                b'\r' => Some(b"\\r"),
                b'\t' => Some(b"\\t"),
                0x08 => Some(b"\\b"),
                0x0C => Some(b"\\f"),
                0x00..=0x1F => None, // \u00XX below
                _ => continue,
            };
            self.buf.extend_from_slice(&bytes[start..i]);
            match esc {
                Some(e) => self.buf.extend_from_slice(e),
                None => {
                    const HEX: &[u8; 16] = b"0123456789abcdef";
                    self.buf.extend_from_slice(b"\\u00");
                    self.buf.push(HEX[(b >> 4) as usize]);
                    self.buf.push(HEX[(b & 0xF) as usize]);
                }
            }
            start = i + 1;
        }
        self.buf.extend_from_slice(&bytes[start..]);
    }

    #[inline]
    pub(crate) fn begin_map(&mut self) {
        self.buf.push(b'{');
        self.has_item.push(false);
    }

    #[inline]
    pub(crate) fn map_key(&mut self, key: &str) {
        self.comma();
        self.write_str(key);
        self.buf.push(b':');
    }

    #[inline]
    pub(crate) fn end_map(&mut self) {
        self.has_item.pop();
        self.buf.push(b'}');
    }

    #[inline]
    pub(crate) fn begin_array(&mut self) {
        self.buf.push(b'[');
        self.has_item.push(false);
    }

    #[inline]
    pub(crate) fn array_item(&mut self) {
        self.comma();
    }

    #[inline]
    pub(crate) fn end_array(&mut self) {
        self.has_item.pop();
        self.buf.push(b']');
    }
}
