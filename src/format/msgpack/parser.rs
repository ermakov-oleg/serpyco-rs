use num_bigint::BigInt;
use pyo3::PyErr;
use smallvec::SmallVec;

use crate::errors::{DecodeError, ToPyErr};
use crate::format::{Kind, ParsedInt, ParsedNumber, VALUE_REPR_LIMIT};
use crate::serde_error::SerdeError;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ContainerKind {
    Array,
    Map,
}

#[derive(Debug, Clone, Copy)]
struct Container {
    kind: ContainerKind,
    remaining: u32,
}

#[derive(Debug, Clone, Copy)]
enum Number {
    I64(i64),
    U64(u64),
    F64(f64),
}

#[derive(Debug)]
pub(crate) struct MsgpackParser<'a> {
    data: &'a [u8],
    index: usize,
    /// Marker byte captured by the last `peek()`. `*_known` readers consume it
    /// without re-reading (their contract already requires a preceding peek) —
    /// mirrors jiter's `last_peek`, and keeps the hot loop free of a second
    /// bounds-check + marker match per value.
    last_marker: u8,
    containers: SmallVec<[Container; 8]>,
    number_text: String,
}

impl<'a> MsgpackParser<'a> {
    pub(crate) fn new(data: &'a [u8]) -> Self {
        Self {
            data,
            index: 0,
            last_marker: 0,
            containers: SmallVec::new(),
            number_text: String::new(),
        }
    }

    #[inline(always)]
    pub(crate) fn peek(&mut self) -> Result<Kind, SerdeError> {
        let marker = self.peek_u8()?;
        self.last_marker = marker;
        match marker {
            0xc0 => Ok(Kind::Null),
            0xc2 | 0xc3 => Ok(Kind::Bool),
            0x00..=0x7f | 0xca..=0xcb | 0xcc..=0xcf | 0xd0..=0xd3 | 0xe0..=0xff => Ok(Kind::Num),
            0xa0..=0xbf | 0xd9..=0xdb => Ok(Kind::Str),
            0xc4..=0xc6 => Ok(Kind::Bytes),
            0x90..=0x9f | 0xdc..=0xdd => Ok(Kind::Array),
            0x80..=0x8f | 0xde..=0xdf => Ok(Kind::Map),
            0xc1 => Err(self.err_at("reserved MessagePack marker", self.index)),
            0xc7..=0xc9 | 0xd4..=0xd8 => {
                Err(self.err_at("MessagePack extension values are not supported", self.index))
            }
        }
    }

    #[inline(always)]
    pub(crate) fn take_null_known(&mut self) -> Result<(), SerdeError> {
        debug_assert_eq!(self.data.get(self.index), Some(&0xc0));
        self.index += 1;
        Ok(())
    }

    #[inline(always)]
    pub(crate) fn take_bool_known(&mut self) -> Result<bool, SerdeError> {
        debug_assert!(matches!(self.data.get(self.index), Some(0xc2 | 0xc3)));
        self.index += 1;
        Ok(self.last_marker == 0xc3)
    }

    #[inline(always)]
    pub(crate) fn take_int_known(&mut self) -> Result<ParsedInt, SerdeError> {
        let pos = self.index;
        match self.read_number()? {
            Number::I64(value) => Ok(ParsedInt::I64(value)),
            Number::U64(value) => {
                if let Ok(value) = i64::try_from(value) {
                    Ok(ParsedInt::I64(value))
                } else {
                    Ok(ParsedInt::Big(BigInt::from(value)))
                }
            }
            Number::F64(_) => {
                self.index = pos;
                Err(self.err_at("expected integer", pos))
            }
        }
    }

    /// Typed number straight from the marker — no text round-trip.
    #[inline(always)]
    pub(crate) fn take_number_known(&mut self) -> Result<ParsedNumber, SerdeError> {
        match self.read_number()? {
            Number::I64(value) => Ok(ParsedNumber::Int(ParsedInt::I64(value))),
            Number::U64(value) => match i64::try_from(value) {
                Ok(value) => Ok(ParsedNumber::Int(ParsedInt::I64(value))),
                Err(_) => Ok(ParsedNumber::Int(ParsedInt::Big(BigInt::from(value)))),
            },
            Number::F64(value) => Ok(ParsedNumber::F64(value)),
        }
    }

    #[inline]
    pub(crate) fn take_number_str_known(&mut self) -> Result<&str, SerdeError> {
        let number = self.read_number()?;
        self.number_text.clear();
        match number {
            Number::I64(value) => {
                let mut buf = itoa::Buffer::new();
                self.number_text.push_str(buf.format(value));
            }
            Number::U64(value) => {
                let mut buf = itoa::Buffer::new();
                self.number_text.push_str(buf.format(value));
            }
            Number::F64(value) => {
                let mut buf = ryu::Buffer::new();
                self.number_text.push_str(buf.format(value));
            }
        }
        Ok(&self.number_text)
    }

    #[inline(always)]
    pub(crate) fn take_str_known(&mut self) -> Result<&'a str, SerdeError> {
        let pos = self.index;
        let len = self.read_str_len()?;
        let bytes = self.take_slice(len)?;
        Self::validate_str(bytes, pos)
    }

    /// ASCII short-circuit first: real-payload strings/keys are short and
    /// overwhelmingly ASCII, where a vectorized `is_ascii` beats a full UTF-8
    /// scan (simdutf8 delegates sub-64-byte inputs to std's scalar DFA).
    #[inline(always)]
    fn validate_str(bytes: &'a [u8], pos: usize) -> Result<&'a str, SerdeError> {
        if bytes.is_ascii() {
            return Ok(unsafe { std::str::from_utf8_unchecked(bytes) });
        }
        simdutf8::basic::from_utf8(bytes)
            .map_err(|_| SerdeError::Py(decode_err("invalid UTF-8 string", pos)))
    }

    /// Materialize the next string straight into a `PyString`, reusing the
    /// ASCII knowledge from validation so CPython never re-scans the bytes.
    #[inline(always)]
    pub(crate) fn take_pystring_known<'py>(
        &mut self,
        py: pyo3::Python<'py>,
    ) -> Result<pyo3::Bound<'py, pyo3::types::PyString>, SerdeError> {
        let pos = self.index;
        let len = self.read_str_len()?;
        let bytes = self.take_slice(len)?;
        if bytes.is_ascii() {
            return Ok(unsafe { crate::python::pystring_ascii_new(py, bytes)? });
        }
        let s = simdutf8::basic::from_utf8(bytes)
            .map_err(|_| SerdeError::Py(decode_err("invalid UTF-8 string", pos)))?;
        Ok(pyo3::types::PyString::new(py, s))
    }

    #[inline(always)]
    pub(crate) fn take_bytes_known(&mut self) -> Result<&'a [u8], SerdeError> {
        let pos = self.index;
        let len = self.read_bin_len()?;
        self.take_slice(len)
            .map_err(|_| self.err_at("truncated binary value", pos))
    }

    #[inline(always)]
    pub(crate) fn enter_map_known(&mut self) -> Result<Option<&'a str>, SerdeError> {
        self.enter_map_inner().map(|(key, _)| key)
    }

    /// `enter_map_known` that also reports the entry count, for a caller that sizes a
    /// dict from it: the returned first key borrows the buffer, so it could not ask
    /// afterwards. `0` means the map was empty.
    #[inline(always)]
    pub(crate) fn enter_map_known_sized(&mut self) -> Result<(Option<&'a str>, usize), SerdeError> {
        self.enter_map_inner()
    }

    #[inline(always)]
    pub(crate) fn enter_array_known(&mut self) -> Result<bool, SerdeError> {
        let len = self.read_array_len()?;
        if len == 0 {
            return Ok(false);
        }
        self.containers.push(Container {
            kind: ContainerKind::Array,
            remaining: len - 1,
        });
        Ok(true)
    }

    /// Entry count of the container the last `enter_*_known` opened, from its header.
    /// Only valid right after entering — it is derived from the entries still pending,
    /// so it shrinks as they are consumed.
    #[inline(always)]
    pub(crate) fn container_len_hint(&self) -> Option<usize> {
        let container = self.containers.last()?;
        Some(self.clamp_stated_len(container.kind, container.remaining as usize + 1))
    }

    /// A header can claim up to 2^32 entries that the input cannot possibly hold, and
    /// the caller turns this count into an allocation, so the bytes left cap it: an
    /// array entry costs at least one byte, a map pair at least two (an empty fixstr
    /// key plus a one-byte value).
    #[inline(always)]
    fn clamp_stated_len(&self, kind: ContainerKind, stated: usize) -> usize {
        let room = self.data.len() - self.index;
        match kind {
            ContainerKind::Array => stated.min(room + 1),
            ContainerKind::Map => stated.min(room / 2 + 1),
        }
    }

    /// Enter a map without a preceding `peek()` (discriminated-union scan).
    #[inline]
    pub(crate) fn enter_map(&mut self) -> Result<Option<&'a str>, SerdeError> {
        let pos = self.index;
        if self.peek()? != Kind::Map {
            return Err(self.err_at("expected map", pos));
        }
        self.enter_map_inner().map(|(key, _)| key)
    }

    #[inline(always)]
    fn enter_map_inner(&mut self) -> Result<(Option<&'a str>, usize), SerdeError> {
        let len = self.read_map_len()?;
        if len == 0 {
            return Ok((None, 0));
        }
        self.containers.push(Container {
            kind: ContainerKind::Map,
            remaining: len - 1,
        });
        let hint = self.clamp_stated_len(ContainerKind::Map, len as usize);
        Ok((Some(self.take_map_key()?), hint))
    }

    #[inline(always)]
    pub(crate) fn next_key(&mut self) -> Result<Option<&'a str>, SerdeError> {
        let has_next = {
            let Some(container) = self.containers.last_mut() else {
                return Err(self.err_at("map iterator is not active", self.index));
            };
            if container.kind != ContainerKind::Map {
                return Err(self.err_at("expected active map", self.index));
            }
            if container.remaining == 0 {
                false
            } else {
                container.remaining -= 1;
                true
            }
        };
        if has_next {
            self.take_map_key().map(Some)
        } else {
            self.containers.pop();
            Ok(None)
        }
    }

    #[inline(always)]
    pub(crate) fn next_array_item(&mut self) -> Result<bool, SerdeError> {
        let Some(container) = self.containers.last_mut() else {
            return Err(self.err_at("array iterator is not active", self.index));
        };
        if container.kind != ContainerKind::Array {
            return Err(self.err_at("expected active array", self.index));
        }
        if container.remaining == 0 {
            self.containers.pop();
            Ok(false)
        } else {
            container.remaining -= 1;
            Ok(true)
        }
    }

    #[inline]
    pub(crate) fn skip_value(&mut self) -> Result<(), SerdeError> {
        let mut pending = 1usize;
        while pending > 0 {
            pending -= 1;
            let marker_pos = self.index;
            let marker = self.take_u8()?;
            match marker {
                0x00..=0x7f | 0x80..=0x8f | 0x90..=0x9f | 0xa0..=0xbf | 0xe0..=0xff => match marker
                {
                    0x80..=0x8f => {
                        add_pending(&mut pending, ((marker & 0x0f) as usize) * 2, marker_pos)?
                    }
                    0x90..=0x9f => add_pending(&mut pending, (marker & 0x0f) as usize, marker_pos)?,
                    0xa0..=0xbf => self.skip_bytes((marker & 0x1f) as usize)?,
                    _ => {}
                },
                0xc0 | 0xc2 | 0xc3 => {}
                0xc1 => return Err(self.err_at("reserved MessagePack marker", marker_pos)),
                0xc4 | 0xd9 => {
                    let len = self.take_u8()? as usize;
                    self.skip_bytes(len)?;
                }
                0xc5 | 0xda => {
                    let len = self.take_u16()? as usize;
                    self.skip_bytes(len)?;
                }
                0xc6 | 0xdb => {
                    let len = usize::try_from(self.take_u32()?)
                        .map_err(|_| self.err_at("value length is too large", marker_pos))?;
                    self.skip_bytes(len)?;
                }
                0xca | 0xce | 0xd2 => self.skip_bytes(4)?,
                0xcb | 0xcf | 0xd3 => self.skip_bytes(8)?,
                0xcc | 0xd0 => self.skip_bytes(1)?,
                0xcd | 0xd1 => self.skip_bytes(2)?,
                0xdc => add_pending(&mut pending, self.take_u16()? as usize, marker_pos)?,
                0xdd => {
                    let len = usize::try_from(self.take_u32()?)
                        .map_err(|_| self.err_at("array length is too large", marker_pos))?;
                    add_pending(&mut pending, len, marker_pos)?;
                }
                0xde => add_pending(&mut pending, self.take_u16()? as usize * 2, marker_pos)?,
                0xdf => {
                    let len = usize::try_from(self.take_u32()?)
                        .map_err(|_| self.err_at("map length is too large", marker_pos))?;
                    add_pending(
                        &mut pending,
                        len.checked_mul(2)
                            .ok_or_else(|| self.err_at("map length is too large", marker_pos))?,
                        marker_pos,
                    )?;
                }
                0xc7 => {
                    let len = self.take_u8()? as usize;
                    self.skip_bytes(len + 1)?;
                }
                0xc8 => {
                    let len = self.take_u16()? as usize;
                    self.skip_bytes(len + 1)?;
                }
                0xc9 => {
                    let len = usize::try_from(self.take_u32()?)
                        .map_err(|_| self.err_at("extension length is too large", marker_pos))?;
                    self.skip_bytes(len.checked_add(1).ok_or_else(|| {
                        self.err_at("extension length is too large", marker_pos)
                    })?)?;
                }
                0xd4 => self.skip_bytes(2)?,
                0xd5 => self.skip_bytes(3)?,
                0xd6 => self.skip_bytes(5)?,
                0xd7 => self.skip_bytes(9)?,
                0xd8 => self.skip_bytes(17)?,
            }
        }
        Ok(())
    }

    #[inline]
    pub(crate) fn take_raw_value(&mut self) -> Result<&'a [u8], SerdeError> {
        let start = self.index;
        self.skip_value()?;
        Ok(&self.data[start..self.index])
    }

    pub(crate) fn take_value_repr(&mut self) -> Result<String, SerdeError> {
        let mut out = String::new();
        self.render_value(&mut out, true, 0)?;
        Ok(out)
    }

    #[inline]
    pub(crate) fn finish(&mut self) -> Result<(), SerdeError> {
        if self.index == self.data.len() {
            Ok(())
        } else {
            Err(self.err_at("trailing data", self.index))
        }
    }

    #[inline(always)]
    fn take_map_key(&mut self) -> Result<&'a str, SerdeError> {
        let pos = self.index;
        match self.peek()? {
            Kind::Str => self.take_str_known(),
            _ => Err(self.err_at("MessagePack map keys must be strings", pos)),
        }
    }

    /// Recursion cap for error rendering. The byte budget alone bounds the
    /// depth at VALUE_REPR_LIMIT frames (each level emits at least one byte),
    /// but that is deeper than a 1 MiB thread stack (Windows main thread on
    /// CPython < 3.13) survives — so the depth is capped explicitly, far below
    /// any useful error text.
    const RENDER_MAX_DEPTH: u32 = 64;

    /// Error-message rendering is budgeted by [`VALUE_REPR_LIMIT`] and
    /// [`Self::RENDER_MAX_DEPTH`]: an attacker must not be able to amplify a
    /// huge or deeply nested value at the mismatch point into an even bigger
    /// error string or a stack overflow. Whatever the budget cuts is still
    /// consumed via the iterative `skip_value`, so the cursor ends up exactly
    /// past the value.
    fn render_value(
        &mut self,
        out: &mut String,
        top_level: bool,
        depth: u32,
    ) -> Result<(), SerdeError> {
        if out.len() >= VALUE_REPR_LIMIT || depth >= Self::RENDER_MAX_DEPTH {
            self.skip_value()?;
            if !out.ends_with('…') {
                out.push('…');
            }
            return Ok(());
        }
        match self.peek()? {
            Kind::Null => {
                self.take_null_known()?;
                out.push_str("None");
            }
            Kind::Bool => {
                out.push_str(if self.take_bool_known()? {
                    "True"
                } else {
                    "False"
                });
            }
            Kind::Num => {
                let raw = self.take_number_str_known()?;
                out.push_str(raw);
            }
            Kind::Str => {
                let value = self.take_str_known()?;
                let shown = truncate_char_boundary(value, VALUE_REPR_LIMIT);
                if top_level {
                    out.push('"');
                    out.push_str(shown);
                    out.push('"');
                } else {
                    push_quoted(out, shown, '\'');
                }
                if shown.len() < value.len() {
                    out.push('…');
                }
            }
            Kind::Bytes => {
                let value = self.take_bytes_known()?;
                let shown = &value[..value.len().min(VALUE_REPR_LIMIT)];
                push_bytes_repr(out, shown);
                if shown.len() < value.len() {
                    out.push('…');
                }
            }
            Kind::Array => {
                out.push('[');
                if self.enter_array_known()? {
                    let mut first = true;
                    loop {
                        if out.len() >= VALUE_REPR_LIMIT {
                            out.push('…');
                            // Consume the current and remaining elements unrendered.
                            loop {
                                self.skip_value()?;
                                if !self.next_array_item()? {
                                    break;
                                }
                            }
                            break;
                        }
                        if !first {
                            out.push_str(", ");
                        }
                        first = false;
                        self.render_value(out, false, depth + 1)?;
                        if !self.next_array_item()? {
                            break;
                        }
                    }
                }
                out.push(']');
            }
            Kind::Map => {
                out.push('{');
                let mut key = self.enter_map_known()?;
                let mut first = true;
                while let Some(k) = key {
                    if out.len() >= VALUE_REPR_LIMIT {
                        out.push('…');
                        // The key is consumed; skip its value and the remaining pairs.
                        loop {
                            self.skip_value()?;
                            if self.next_key()?.is_none() {
                                break;
                            }
                        }
                        break;
                    }
                    if !first {
                        out.push_str(", ");
                    }
                    first = false;
                    let shown = truncate_char_boundary(k, VALUE_REPR_LIMIT);
                    push_quoted(out, shown, '\'');
                    if shown.len() < k.len() {
                        out.push('…');
                    }
                    out.push_str(": ");
                    self.render_value(out, false, depth + 1)?;
                    key = self.next_key()?;
                }
                out.push('}');
            }
        }
        Ok(())
    }

    /// `*_known` only: consumes the marker captured by the preceding `peek()`.
    #[inline(always)]
    fn read_number(&mut self) -> Result<Number, SerdeError> {
        let pos = self.index;
        let marker = self.last_marker;
        self.index += 1;
        match marker {
            0x00..=0x7f => Ok(Number::I64(marker as i64)),
            0xe0..=0xff => Ok(Number::I64(marker as i8 as i64)),
            0xcc => Ok(Number::I64(self.take_u8()? as i64)),
            0xcd => Ok(Number::I64(self.take_u16()? as i64)),
            0xce => Ok(Number::I64(self.take_u32()? as i64)),
            0xcf => Ok(Number::U64(self.take_u64()?)),
            0xd0 => Ok(Number::I64(self.take_u8()? as i8 as i64)),
            0xd1 => Ok(Number::I64(self.take_u16()? as i16 as i64)),
            0xd2 => Ok(Number::I64(self.take_u32()? as i32 as i64)),
            0xd3 => Ok(Number::I64(self.take_u64()? as i64)),
            0xca => Ok(Number::F64(f32::from_bits(self.take_u32()?) as f64)),
            0xcb => Ok(Number::F64(f64::from_bits(self.take_u64()?))),
            _ => {
                self.index = pos;
                Err(self.err_at("expected number", pos))
            }
        }
    }

    /// `*_known` only: consumes the marker captured by the preceding `peek()`.
    #[inline(always)]
    fn read_str_len(&mut self) -> Result<usize, SerdeError> {
        let pos = self.index;
        let marker = self.last_marker;
        self.index += 1;
        match marker {
            marker @ 0xa0..=0xbf => Ok((marker & 0x1f) as usize),
            0xd9 => Ok(self.take_u8()? as usize),
            0xda => Ok(self.take_u16()? as usize),
            0xdb => usize::try_from(self.take_u32()?)
                .map_err(|_| self.err_at("string length is too large", pos)),
            _ => Err(self.err_at("expected string", pos)),
        }
    }

    /// `*_known` only: consumes the marker captured by the preceding `peek()`.
    #[inline(always)]
    fn read_bin_len(&mut self) -> Result<usize, SerdeError> {
        let pos = self.index;
        let marker = self.last_marker;
        self.index += 1;
        match marker {
            0xc4 => Ok(self.take_u8()? as usize),
            0xc5 => Ok(self.take_u16()? as usize),
            0xc6 => usize::try_from(self.take_u32()?)
                .map_err(|_| self.err_at("binary length is too large", pos)),
            _ => Err(self.err_at("expected binary value", pos)),
        }
    }

    /// `*_known` only: consumes the marker captured by the preceding `peek()`.
    #[inline(always)]
    fn read_array_len(&mut self) -> Result<u32, SerdeError> {
        let pos = self.index;
        let marker = self.last_marker;
        self.index += 1;
        match marker {
            marker @ 0x90..=0x9f => Ok((marker & 0x0f) as u32),
            0xdc => Ok(self.take_u16()? as u32),
            0xdd => self.take_u32(),
            _ => Err(self.err_at("expected array", pos)),
        }
    }

    /// `*_known` only: consumes the marker captured by the preceding `peek()`.
    #[inline(always)]
    fn read_map_len(&mut self) -> Result<u32, SerdeError> {
        let pos = self.index;
        let marker = self.last_marker;
        self.index += 1;
        match marker {
            marker @ 0x80..=0x8f => Ok((marker & 0x0f) as u32),
            0xde => Ok(self.take_u16()? as u32),
            0xdf => self.take_u32(),
            _ => Err(self.err_at("expected map", pos)),
        }
    }

    #[inline(always)]
    fn peek_u8(&self) -> Result<u8, SerdeError> {
        self.data
            .get(self.index)
            .copied()
            .ok_or_else(|| self.err_at("unexpected end of MessagePack input", self.index))
    }

    #[inline(always)]
    fn take_u8(&mut self) -> Result<u8, SerdeError> {
        let value = self.peek_u8()?;
        self.index += 1;
        Ok(value)
    }

    #[inline(always)]
    fn take_u16(&mut self) -> Result<u16, SerdeError> {
        let bytes: [u8; 2] = self
            .take_slice(2)?
            .try_into()
            .expect("slice length checked");
        Ok(u16::from_be_bytes(bytes))
    }

    #[inline(always)]
    fn take_u32(&mut self) -> Result<u32, SerdeError> {
        let bytes: [u8; 4] = self
            .take_slice(4)?
            .try_into()
            .expect("slice length checked");
        Ok(u32::from_be_bytes(bytes))
    }

    #[inline(always)]
    fn take_u64(&mut self) -> Result<u64, SerdeError> {
        let bytes: [u8; 8] = self
            .take_slice(8)?
            .try_into()
            .expect("slice length checked");
        Ok(u64::from_be_bytes(bytes))
    }

    #[inline(always)]
    fn take_slice(&mut self, len: usize) -> Result<&'a [u8], SerdeError> {
        let end = self
            .index
            .checked_add(len)
            .ok_or_else(|| self.err_at("value length is too large", self.index))?;
        let slice = self
            .data
            .get(self.index..end)
            .ok_or_else(|| self.err_at("unexpected end of MessagePack input", self.index))?;
        self.index = end;
        Ok(slice)
    }

    #[inline]
    fn skip_bytes(&mut self, len: usize) -> Result<(), SerdeError> {
        self.take_slice(len).map(|_| ())
    }

    #[inline]
    fn err_at(&self, message: &str, position: usize) -> SerdeError {
        SerdeError::Py(decode_err(message, position))
    }
}

#[inline]
fn add_pending(pending: &mut usize, count: usize, marker_pos: usize) -> Result<(), SerdeError> {
    *pending = pending.checked_add(count).ok_or_else(|| {
        SerdeError::Py(decode_err("MessagePack container is too large", marker_pos))
    })?;
    Ok(())
}

#[inline]
fn decode_err(message: &str, position: usize) -> PyErr {
    DecodeError::new_err((message.to_owned(), position))
}

/// Longest prefix of `value` at most `max` bytes long that ends on a char boundary.
fn truncate_char_boundary(value: &str, max: usize) -> &str {
    if value.len() <= max {
        return value;
    }
    let mut end = max;
    while !value.is_char_boundary(end) {
        end -= 1;
    }
    &value[..end]
}

fn push_quoted(out: &mut String, value: &str, quote: char) {
    out.push(quote);
    for ch in value.chars() {
        match ch {
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            ch if ch == quote => {
                out.push('\\');
                out.push(ch);
            }
            ch if ch.is_control() => {
                use std::fmt::Write;
                write!(out, "\\x{:02x}", ch as u32).expect("writing to String cannot fail");
            }
            ch => out.push(ch),
        }
    }
    out.push(quote);
}

fn push_bytes_repr(out: &mut String, value: &[u8]) {
    use std::fmt::Write;

    out.push_str("b'");
    for &byte in value {
        match byte {
            b'\\' => out.push_str("\\\\"),
            b'\'' => out.push_str("\\'"),
            b'\n' => out.push_str("\\n"),
            b'\r' => out.push_str("\\r"),
            b'\t' => out.push_str("\\t"),
            0x20..=0x7e => out.push(byte as char),
            _ => write!(out, "\\x{byte:02x}").expect("writing to String cannot fail"),
        }
    }
    out.push('\'');
}
