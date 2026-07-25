/// Streaming JSON writer over a `Vec<u8>`.
///
/// Separators are trailing: every element is followed by `item_end`, and the
/// container's closer drops the one comma left over. That keeps the per-element
/// path branch-free (an unconditional push) and needs no open-container stack —
/// the alternative, deciding "is this the first element?" per item, costs a
/// branch and a stack slot on the hottest path in the writer.
///
/// Call contract: `map_key`/`map_key_encoded` before each map value, then
/// `item_end` after each map value and after each array element.
#[derive(Debug)]
pub(crate) struct JsonWriter {
    buf: Vec<u8>,
}

/// A saved writer position for a speculative write that may be rolled back
/// (union member probing, omit_none null-skip). The buffer length alone captures
/// the whole writer state, so rollback is a truncate.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Checkpoint {
    buf_len: usize,
}

/// Length below which the inline scan beats the SIMD escape call (one AVX2 block).
const SHORT_STR: usize = 32;

/// The exact set JSON requires escaping: `"`, `\` and control chars.
#[inline(always)]
fn needs_escape(b: u8) -> bool {
    b < 0x20 || b == b'"' || b == b'\\'
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
    pub(crate) fn with_capacity(capacity: usize) -> Self {
        JsonWriter {
            buf: Vec::with_capacity(capacity),
        }
    }

    #[inline]
    pub(crate) fn as_bytes(&self) -> &[u8] {
        &self.buf
    }

    /// Terminate the element just written. The closer strips the trailing comma,
    /// so this stays an unconditional push.
    #[inline]
    pub(crate) fn item_end(&mut self) {
        self.buf.push(b',');
    }

    /// Close a container: drop the comma left by the last element, if any.
    /// Only this writer's own separator can be the final byte — a nested value
    /// always ends in `"`, a digit, `e`, `l` (null/true), `}` or `]`.
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
        }
    }

    /// Undo everything written since `cp` (a failed union member, an omit_none
    /// value that turned out null).
    #[inline(always)]
    pub(crate) fn rollback(&mut self, cp: Checkpoint) {
        self.buf.truncate(cp.buf_len);
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
        let bytes = v.as_bytes();
        // One growth check for the whole string instead of one per push/extend.
        self.buf.reserve(bytes.len() + 2);
        self.buf.push(b'"');
        // Real payloads are dominated by short, escape-free strings (ids, enum
        // members, names). `escape_bytes` is a `#[target_feature(avx2)]` call on
        // x86_64, so it can never inline into this function and pays call + vector
        // setup per string; this scan compiles to inlined baseline SIMD and copies
        // verbatim. Longer strings amortize the call, so they go to the SIMD path.
        if bytes.len() <= SHORT_STR && !bytes.iter().any(|&b| needs_escape(b)) {
            self.buf.extend_from_slice(bytes);
        } else {
            // Escapes exactly ", \, and control chars (<0x20) — byte-identical to
            // json.dumps/serde_json (no `/` or U+2028/2029 escaping).
            v_jsonescape::escape_bytes(v, &mut self.buf);
        }
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

    /// Append a key already rendered by [`encode_map_key`] (quotes, escaping and
    /// the `:` included) — the escape pass is skipped entirely.
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
