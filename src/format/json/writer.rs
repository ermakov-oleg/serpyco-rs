/// Streaming JSON writer over a `Vec<u8>`.
///
/// Separators are trailing: every element is followed by `item_end`, and the
/// container's closer drops the one comma left over. That keeps the per-element
/// path an unconditional push and needs no open-container stack.
///
/// Call contract: `map_key`/`map_key_encoded` before each map value, then
/// `item_end` after each map value and after each array element.
#[derive(Debug)]
pub(crate) struct JsonWriter {
    buf: Vec<u8>,
}

/// A saved writer position for a speculative write that may be rolled back
/// (union member probing, omit_none null-skip). The buffer length alone captures
/// the whole state, so rollback is a truncate.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Checkpoint {
    buf_len: usize,
}

/// The exact set JSON requires escaping: `"`, `\` and control chars.
#[inline(always)]
fn needs_escape(b: u8) -> bool {
    b < 0x20 || b == b'"' || b == b'\\'
}

const LANE: u64 = 0x0101_0101_0101_0101;
const HIGH: u64 = 0x8080_8080_8080_8080;

/// Per-byte SWAR predicate: high bit set in every lane holding a byte that needs
/// escaping. Bytes >= 0x80 (UTF-8 continuations) never match — `!word` clears
/// their lane — so multi-byte sequences pass through untouched.
///
/// SWAR rather than a SIMD crate: a `#[target_feature(avx2)]` escape cannot be
/// inlined into callers that lack the feature, so every string paid a call.
///
/// ONLY THE LOWEST SET LANE IS TRUSTWORTHY: a borrow out of lane k can spuriously
/// set lane k+1, and such a borrow happens exactly when lane k itself matches, so
/// false positives always sit *above* a true one. Hence the caller must take one
/// byte per word and recompute.
#[inline(always)]
fn escape_lanes(word: u64) -> u64 {
    let below_0x20 = word.wrapping_sub(0x20 * LANE) & !word;
    let quote = word ^ (0x22 * LANE);
    let quote = quote.wrapping_sub(LANE) & !quote;
    let backslash = word ^ (0x5C * LANE);
    let backslash = backslash.wrapping_sub(LANE) & !backslash;
    (below_0x20 | quote | backslash) & HIGH
}

/// Write one byte in its JSON-escaped form. Matches `json.dumps`/serde_json:
/// short forms for `\b\t\n\f\r`, lowercase `\u00XX` for the other controls.
#[cold]
#[inline(never)]
fn write_escaped_byte(buf: &mut Vec<u8>, b: u8) {
    match b {
        b'"' => buf.extend_from_slice(br#"\""#),
        b'\\' => buf.extend_from_slice(br"\\"),
        0x08 => buf.extend_from_slice(br"\b"),
        0x09 => buf.extend_from_slice(br"\t"),
        0x0A => buf.extend_from_slice(br"\n"),
        0x0C => buf.extend_from_slice(br"\f"),
        0x0D => buf.extend_from_slice(br"\r"),
        _ => {
            const HEX: &[u8; 16] = b"0123456789abcdef";
            buf.extend_from_slice(&[
                b'\\',
                b'u',
                b'0',
                b'0',
                HEX[(b >> 4) as usize],
                HEX[(b & 0x0F) as usize],
            ]);
        }
    }
}

/// Append `s` JSON-escaped (without the surrounding quotes), copying clean runs
/// in bulk and escaping byte by byte only where needed.
fn escape_into(buf: &mut Vec<u8>, s: &str) {
    let bytes = s.as_bytes();
    let mut clean_from = 0;
    let mut i = 0;

    while i + 8 <= bytes.len() {
        let word = u64::from_le_bytes(bytes[i..i + 8].try_into().expect("8-byte chunk"));
        let lanes = escape_lanes(word);
        if lanes == 0 {
            i += 8;
            continue;
        }
        let at = i + (lanes.trailing_zeros() / 8) as usize;
        buf.extend_from_slice(&bytes[clean_from..at]);
        write_escaped_byte(buf, bytes[at]);
        i = at + 1;
        clean_from = i;
    }

    while i < bytes.len() {
        let b = bytes[i];
        if needs_escape(b) {
            buf.extend_from_slice(&bytes[clean_from..i]);
            write_escaped_byte(buf, b);
            clean_from = i + 1;
        }
        i += 1;
    }
    buf.extend_from_slice(&bytes[clean_from..]);
}

/// Render `"key":` once, at encoder-construction time. Escaping still runs:
/// a JSON key can come from an `Alias`, not just a Python identifier.
pub(crate) fn encode_map_key(key: &str) -> Box<[u8]> {
    let mut buf = Vec::with_capacity(key.len() + 3);
    buf.push(b'"');
    escape_into(&mut buf, key);
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
        escape_into(&mut self.buf, v);
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
    /// the `:` included).
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
