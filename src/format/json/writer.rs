/// Streaming JSON writer: writes into Vec<u8> and manages commas itself.
/// Call contract: map_key before each map value, array_item before each array element.
#[derive(Debug)]
pub(crate) struct JsonWriter {
    buf: Vec<u8>,
    /// true = the current container already has an element (comma needed).
    has_item: Vec<bool>,
}

/// A saved writer position for a speculative write that may be rolled back
/// (union member probing, omit_none null-skip). Captures buffer length and the
/// container's comma state so `rollback` fully undoes it without a probe buffer + splice.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Checkpoint {
    buf_len: usize,
    has_item_len: usize,
    top_had_item: bool,
}

/// Render `"key":` once for a map key fixed at encoder-construction time.
/// Escaping still runs here — a JSON key can come from an `Alias`, not just a
/// Python identifier — but it runs once per encoder instead of once per dump.
pub(crate) fn encode_map_key(key: &str) -> Box<[u8]> {
    let mut buf = Vec::with_capacity(key.len() + 3);
    buf.push(b'"');
    v_jsonescape::escape_bytes(key, &mut buf);
    buf.extend_from_slice(b"\":");
    buf.into_boxed_slice()
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
    /// container-nesting/comma state (dropping a failed member's or omit_none value's output).
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
        // SIMD JSON string escaping (AVX2/SSE2 on x86_64, NEON on aarch64; scalar
        // fallback). Escapes exactly ", \, and control chars (<0x20) — byte-
        // identical to json.dumps/serde_json (no `/` or U+2028/2029 escaping).
        v_jsonescape::escape_bytes(v, &mut self.buf);
        self.buf.push(b'"');
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

    /// Append a key already rendered by [`encode_map_key`] (quotes, escaping and
    /// the `:` included) — the escape pass is skipped entirely.
    #[inline]
    pub(crate) fn map_key_encoded(&mut self, encoded: &[u8]) {
        self.comma();
        self.buf.extend_from_slice(encoded);
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
