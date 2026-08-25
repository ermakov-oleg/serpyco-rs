//! Minimal JSON scanner for the specialized GitHub-issue codec (benchmark only).
//!
//! Reads the raw `&[u8]` and materializes Python objects straight from it: no
//! tokenizer object, no `Value` tree, no intermediate `dict`. Every reader is
//! called from code that already knows which JSON type the schema expects, so
//! there is no `peek -> dispatch` layer either — the callers branch on the lead
//! byte themselves where the schema allows more than one shape (optional, union).

use std::os::raw::c_char;

use pyo3::exceptions::PyValueError;
use pyo3::{ffi, PyErr, PyResult, Python};
use smallvec::SmallVec;

use super::obj::Obj;
use super::simd::{find, Stop};

/// Nesting cap for `skip_value`; the real codec derives this from
/// `max_recursion_depth`, the prototype hard-codes it.
const MAX_SKIP_DEPTH: u32 = 64;

#[cold]
#[inline(never)]
pub(super) fn syntax(msg: &'static str, at: usize) -> PyErr {
    PyValueError::new_err(format!("{msg} at position {at}"))
}

/// A string token still in the input buffer.
pub(super) enum StrTok<'a> {
    /// No escapes, every byte < 0x80 — copy straight into a compact-ASCII `str`.
    Ascii(&'a [u8]),
    /// No escapes, some byte >= 0x80 — hand the span to CPython's UTF-8 decoder.
    Utf8(&'a [u8]),
    /// Contains at least one `\` — the span still holds the escapes.
    Escaped(&'a [u8]),
}

pub(super) struct Scan<'a> {
    d: &'a [u8],
    pub(super) i: usize,
}

impl<'a> Scan<'a> {
    #[inline]
    pub(super) fn new(d: &'a [u8]) -> Self {
        Scan { d, i: 0 }
    }

    /// 0 past the end: no JSON token starts with NUL, so every caller's byte
    /// match rejects it without a separate bounds check.
    #[inline(always)]
    pub(super) fn cur(&self) -> u8 {
        if self.i < self.d.len() {
            self.d[self.i]
        } else {
            0
        }
    }

    /// Skip inter-token whitespace. Outside a string every byte <= 0x20 that
    /// valid JSON allows is whitespace, so one comparison per byte is enough;
    /// an illegal control byte here is rejected by the next token check anyway.
    #[inline(always)]
    pub(super) fn ws(&mut self) {
        let d = self.d;
        let mut i = self.i;
        while i < d.len() && d[i] <= b' ' {
            i += 1;
        }
        self.i = i;
    }

    /// Trailing bytes after the top-level value (whitespace only).
    #[inline]
    pub(super) fn finish(&mut self) -> PyResult<()> {
        self.ws();
        if self.i == self.d.len() {
            Ok(())
        } else {
            Err(syntax("trailing data", self.i))
        }
    }

    #[inline]
    pub(super) fn expect(&mut self, b: u8, msg: &'static str) -> PyResult<()> {
        if self.cur() == b {
            self.i += 1;
            Ok(())
        } else {
            Err(syntax(msg, self.i))
        }
    }

    // --- objects -------------------------------------------------------

    /// Consume `{`. Callers then drive `next_key`.
    #[inline]
    pub(super) fn enter_object(&mut self) -> PyResult<()> {
        self.expect(b'{', "expected '{'")
    }

    /// Next member key of the current object, cursor left on its value.
    /// `None` — the object ended (its `}` is consumed).
    ///
    /// The key is returned *raw*: a key carrying escapes keeps its backslashes
    /// and therefore matches no field name, i.e. it is treated as unknown.
    #[inline(always)]
    pub(super) fn next_key(&mut self, first: bool) -> PyResult<Option<&'a [u8]>> {
        self.ws();
        match self.cur() {
            b'}' => {
                self.i += 1;
                return Ok(None);
            }
            b',' if !first => {
                self.i += 1;
                self.ws();
            }
            _ if first => {}
            _ => return Err(syntax("expected ',' or '}'", self.i)),
        }
        if self.cur() != b'"' {
            return Err(syntax("expected object key", self.i));
        }
        let key = match self.ascii_span() {
            Some(s) => s,
            None => match self.scan_string_slow()? {
                StrTok::Ascii(s) | StrTok::Utf8(s) | StrTok::Escaped(s) => s,
            },
        };
        self.ws();
        self.expect(b':', "expected ':'")?;
        self.ws();
        Ok(Some(key))
    }

    /// Oracle path: the key at this position is known, so step over it by its
    /// length instead of scanning and comparing it. The closing quote and `:`
    /// are still checked, so a document that does not match the plan is
    /// rejected rather than silently misread.
    #[inline]
    pub(super) fn skip_known_key(&mut self, key_len: usize, first: bool) -> PyResult<()> {
        self.ws();
        if !first {
            self.expect(b',', "ordered plan: expected ','")?;
            self.ws();
        }
        self.expect(b'"', "ordered plan: expected key")?;
        self.i += key_len;
        self.expect(b'"', "ordered plan: key length mismatch")?;
        self.ws();
        self.expect(b':', "ordered plan: expected ':'")?;
        self.ws();
        Ok(())
    }

    // --- arrays --------------------------------------------------------

    #[inline]
    pub(super) fn enter_array(&mut self) -> PyResult<()> {
        self.expect(b'[', "expected '['")
    }

    /// `true` — an element follows; `false` — the array ended (`]` consumed).
    #[inline(always)]
    pub(super) fn next_item(&mut self, first: bool) -> PyResult<bool> {
        self.ws();
        match self.cur() {
            b']' => {
                self.i += 1;
                Ok(false)
            }
            b',' if !first => {
                self.i += 1;
                self.ws();
                Ok(true)
            }
            _ if first => Ok(true),
            _ => Err(syntax("expected ',' or ']'", self.i)),
        }
    }

    // --- scalars -------------------------------------------------------

    #[inline(always)]
    pub(super) fn take_null(&mut self) -> PyResult<()> {
        if self.d[self.i..].starts_with(b"null") {
            self.i += 4;
            Ok(())
        } else {
            Err(syntax("expected null", self.i))
        }
    }

    #[inline(always)]
    pub(super) fn take_bool(&mut self) -> PyResult<bool> {
        if self.d[self.i..].starts_with(b"true") {
            self.i += 4;
            Ok(true)
        } else if self.d[self.i..].starts_with(b"false") {
            self.i += 5;
            Ok(false)
        } else {
            Err(syntax("expected bool", self.i))
        }
    }

    /// Integer token -> `i64`. A float-shaped token or a magnitude beyond `i64`
    /// is an error here; the real codec widens to `BigInt`/float.
    #[inline(always)]
    pub(super) fn take_i64(&mut self) -> PyResult<i64> {
        let start = self.i;
        let d = self.d;
        let neg = self.cur() == b'-';
        if neg {
            self.i += 1;
        }
        let digits_from = self.i;
        let mut acc: u64 = 0;
        while self.i < d.len() {
            let c = d[self.i].wrapping_sub(b'0');
            if c > 9 {
                break;
            }
            acc = acc.wrapping_mul(10).wrapping_add(c as u64);
            self.i += 1;
        }
        let ndigits = self.i - digits_from;
        if ndigits == 0 || ndigits > 19 {
            return Err(syntax("expected integer", start));
        }
        if matches!(self.cur(), b'.' | b'e' | b'E') {
            return Err(syntax("expected integer, found float", start));
        }
        if neg {
            if acc > (i64::MAX as u64) + 1 {
                return Err(syntax("integer out of range", start));
            }
            Ok((acc as i64).wrapping_neg())
        } else {
            if acc > i64::MAX as u64 {
                return Err(syntax("integer out of range", start));
            }
            Ok(acc as i64)
        }
    }

    /// Fast path: the span of a `"`-led string with no escapes and no
    /// non-ASCII byte, which is what nearly every key and value in this payload
    /// is. `None` leaves the cursor untouched for the cold path to redo.
    ///
    /// Kept separate and `#[inline(always)]` so the callers get the scan loop
    /// inlined; the general reader below is a real (cold) call.
    #[inline(always)]
    fn ascii_span(&mut self) -> Option<&'a [u8]> {
        let start = self.i + 1;
        let at = find(Stop::StringEnd, self.d, start)?;
        if self.d[at] != b'"' {
            return None;
        }
        self.i = at + 1;
        Some(&self.d[start..at])
    }

    /// Scan a `"`-led string token, leaving the cursor after the closing quote.
    #[inline(always)]
    pub(super) fn scan_string(&mut self) -> PyResult<StrTok<'a>> {
        debug_assert_eq!(self.cur(), b'"');
        match self.ascii_span() {
            Some(s) => Ok(StrTok::Ascii(s)),
            None => self.scan_string_slow(),
        }
    }

    /// Everything the fast path bailed on: a non-ASCII byte, an escape, or an
    /// unterminated string.
    #[cold]
    #[inline(never)]
    fn scan_string_slow(&mut self) -> PyResult<StrTok<'a>> {
        let start = self.i + 1;
        let Some(at) = find(Stop::StringEnd, self.d, start) else {
            return Err(syntax("unterminated string", start));
        };
        match self.d[at] {
            b'"' => {
                self.i = at + 1;
                Ok(StrTok::Ascii(&self.d[start..at]))
            }
            b'\\' => self.scan_escaped(start),
            // Non-ASCII: only `"` and `\` still matter from here on.
            _ => match find(Stop::QuoteOrBackslash, self.d, at + 1) {
                Some(end) if self.d[end] == b'"' => {
                    self.i = end + 1;
                    Ok(StrTok::Utf8(&self.d[start..end]))
                }
                Some(_) => self.scan_escaped(start),
                None => Err(syntax("unterminated string", start)),
            },
        }
    }

    /// A `\` was seen: walk to the closing quote escape-aware and hand the raw
    /// span (escapes included) back for [`unescape`].
    #[cold]
    fn scan_escaped(&mut self, start: usize) -> PyResult<StrTok<'a>> {
        let mut i = start;
        loop {
            let Some(at) = find(Stop::QuoteOrBackslash, self.d, i) else {
                return Err(syntax("unterminated string", start));
            };
            if self.d[at] == b'"' {
                self.i = at + 1;
                return Ok(StrTok::Escaped(&self.d[start..at]));
            }
            i = at + 2; // skip the escaped byte
        }
    }

    // --- python materialization ----------------------------------------

    /// `"..."` -> `str`.
    #[inline(always)]
    pub(super) fn take_str(&mut self, py: Python<'_>) -> PyResult<Obj> {
        if self.cur() != b'"' {
            return Err(syntax("expected string", self.i));
        }
        match self.ascii_span() {
            Some(s) => unsafe { new_ascii_str(py, s) },
            None => self.take_str_slow(py),
        }
    }

    #[cold]
    #[inline(never)]
    fn take_str_slow(&mut self, py: Python<'_>) -> PyResult<Obj> {
        match self.scan_string_slow()? {
            StrTok::Ascii(s) => unsafe { new_ascii_str(py, s) },
            StrTok::Utf8(s) => unsafe { new_utf8_str(py, s) },
            StrTok::Escaped(s) => {
                let mut buf: SmallVec<[u8; 128]> = SmallVec::new();
                unescape(s, &mut buf)?;
                unsafe { new_str(py, &buf) }
            }
        }
    }

    /// Bytes of a string token, unescaped into `buf` when needed. Used where the
    /// text is consumed in Rust (enum lookup, datetime) rather than handed to Python.
    #[inline]
    pub(super) fn take_str_bytes<'b>(
        &mut self,
        buf: &'b mut SmallVec<[u8; 64]>,
    ) -> PyResult<&'b [u8]>
    where
        'a: 'b,
    {
        if self.cur() != b'"' {
            return Err(syntax("expected string", self.i));
        }
        Ok(match self.scan_string()? {
            StrTok::Ascii(s) | StrTok::Utf8(s) => s,
            StrTok::Escaped(s) => {
                unescape(s, buf)?;
                &buf[..]
            }
        })
    }

    /// Skip one value of any shape (unknown key).
    pub(super) fn skip_value(&mut self) -> PyResult<()> {
        self.skip_value_at(0)
    }

    fn skip_value_at(&mut self, depth: u32) -> PyResult<()> {
        if depth > MAX_SKIP_DEPTH {
            return Err(syntax("recursion limit exceeded", self.i));
        }
        match self.cur() {
            b'"' => {
                self.scan_string()?;
                Ok(())
            }
            b'{' => {
                self.i += 1;
                let mut first = true;
                while self.next_key(first)?.is_some() {
                    self.skip_value_at(depth + 1)?;
                    first = false;
                }
                Ok(())
            }
            b'[' => {
                self.i += 1;
                let mut first = true;
                while self.next_item(first)? {
                    self.skip_value_at(depth + 1)?;
                    first = false;
                }
                Ok(())
            }
            b't' | b'f' => {
                self.take_bool()?;
                Ok(())
            }
            b'n' => self.take_null(),
            b'-' | b'0'..=b'9' => {
                self.i += 1;
                while self.i < self.d.len()
                    && matches!(
                        self.d[self.i],
                        b'0'..=b'9' | b'.' | b'e' | b'E' | b'+' | b'-'
                    )
                {
                    self.i += 1;
                }
                Ok(())
            }
            _ => Err(syntax("unexpected token", self.i)),
        }
    }
}

/// Decode JSON escapes into `buf`.
fn unescape<A: smallvec::Array<Item = u8>>(s: &[u8], buf: &mut SmallVec<A>) -> PyResult<()> {
    buf.clear();
    buf.reserve(s.len());
    let mut i = 0;
    while i < s.len() {
        let b = s[i];
        if b != b'\\' {
            let from = i;
            while i < s.len() && s[i] != b'\\' {
                i += 1;
            }
            buf.extend_from_slice(&s[from..i]);
            continue;
        }
        i += 1;
        let e = *s.get(i).ok_or_else(|| syntax("truncated escape", i))?;
        i += 1;
        match e {
            b'"' => buf.push(b'"'),
            b'\\' => buf.push(b'\\'),
            b'/' => buf.push(b'/'),
            b'b' => buf.push(0x08),
            b'f' => buf.push(0x0C),
            b'n' => buf.push(b'\n'),
            b'r' => buf.push(b'\r'),
            b't' => buf.push(b'\t'),
            b'u' => {
                let mut cp = read_hex4(s, i)? as u32;
                i += 4;
                if (0xD800..0xDC00).contains(&cp) {
                    // High surrogate: a following low surrogate completes the pair.
                    if s.get(i) == Some(&b'\\') && s.get(i + 1) == Some(&b'u') {
                        let lo = read_hex4(s, i + 2)? as u32;
                        if (0xDC00..0xE000).contains(&lo) {
                            cp = 0x1_0000 + ((cp - 0xD800) << 10) + (lo - 0xDC00);
                            i += 6;
                        }
                    }
                }
                // Lone surrogates are replaced rather than rejected (prototype).
                let ch = char::from_u32(cp).unwrap_or('\u{FFFD}');
                let mut tmp = [0u8; 4];
                buf.extend_from_slice(ch.encode_utf8(&mut tmp).as_bytes());
            }
            _ => return Err(syntax("invalid escape", i)),
        }
    }
    Ok(())
}

#[inline]
fn read_hex4(s: &[u8], at: usize) -> PyResult<u16> {
    let chunk = s
        .get(at..at + 4)
        .ok_or_else(|| syntax("truncated \\u escape", at))?;
    let mut v: u16 = 0;
    for &c in chunk {
        let d = match c {
            b'0'..=b'9' => c - b'0',
            b'a'..=b'f' => c - b'a' + 10,
            b'A'..=b'F' => c - b'A' + 10,
            _ => return Err(syntax("invalid \\u escape", at)),
        };
        v = (v << 4) | d as u16;
    }
    Ok(v)
}

/// # Safety
/// Every byte of `b` must be < 0x80.
#[inline(always)]
pub(super) unsafe fn new_ascii_str(py: Python<'_>, b: &[u8]) -> PyResult<Obj> {
    let o = ffi::PyUnicode_New(b.len() as ffi::Py_ssize_t, 127);
    if o.is_null() {
        return Err(PyErr::fetch(py));
    }
    debug_assert!(b.is_ascii());
    std::ptr::copy_nonoverlapping(b.as_ptr(), ffi::PyUnicode_1BYTE_DATA(o), b.len());
    Ok(Obj::from_owned(o))
}

#[inline(always)]
unsafe fn new_utf8_str(py: Python<'_>, b: &[u8]) -> PyResult<Obj> {
    let o = ffi::PyUnicode_DecodeUTF8(
        b.as_ptr() as *const c_char,
        b.len() as ffi::Py_ssize_t,
        std::ptr::null(),
    );
    if o.is_null() {
        return Err(PyErr::fetch(py));
    }
    Ok(Obj::from_owned(o))
}

#[inline(always)]
unsafe fn new_str(py: Python<'_>, b: &[u8]) -> PyResult<Obj> {
    if b.is_ascii() {
        new_ascii_str(py, b)
    } else {
        new_utf8_str(py, b)
    }
}
