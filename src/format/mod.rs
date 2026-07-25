pub(crate) mod bridge;
pub(crate) mod json;

use pyo3::exceptions::PyValueError;
use pyo3::PyErr;

use json::parser::{JsonParser, ParsedInt};
use json::writer::{Checkpoint as JsonCheckpoint, JsonWriter};

use crate::serde_error::SerdeError;

/// Kind of the next value in the input stream (format-agnostic).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Kind {
    Null,
    Bool,
    Num,
    Str,
    Array,
    Map,
}

pub(crate) const FORMAT_JSON: u8 = 0;

/// Enum dispatch instead of dyn: closed set of formats;
/// a new format is a new variant plus method implementations.
#[derive(Debug)]
pub(crate) enum Writer {
    Json(JsonWriter),
}

/// Opaque saved writer position (per-format), paired with `Writer::checkpoint` /
/// `Writer::rollback`. A new format adds its own variant.
#[derive(Debug, Clone, Copy)]
pub(crate) enum Checkpoint {
    Json(JsonCheckpoint),
}

/// A map key rendered per format once, at encoder-construction time. Entity and
/// TypedDict keys are fixed by the type, so escaping them on every dump is pure
/// waste; the hot path copies these bytes verbatim. A new format adds its own
/// pre-rendered field here.
#[derive(Debug, Clone)]
pub(crate) struct EncodedKey {
    json: Box<[u8]>,
}

impl EncodedKey {
    pub(crate) fn new(key: &str) -> Self {
        EncodedKey {
            json: json::writer::encode_map_key(key),
        }
    }
}

impl Writer {
    pub(crate) fn new(format: u8) -> Result<Self, PyErr> {
        match format {
            FORMAT_JSON => Ok(Writer::Json(JsonWriter::new())),
            _ => Err(PyValueError::new_err(format!(
                "unknown format id: {format}"
            ))),
        }
    }

    #[inline]
    pub(crate) fn as_bytes(&self) -> &[u8] {
        match self {
            Writer::Json(w) => w.as_bytes(),
        }
    }

    /// Snapshot the writer so a speculative value (union member probe, omit_none
    /// value) can be rolled back in place — no separate probe buffer or splice.
    #[inline(always)]
    pub(crate) fn checkpoint(&self) -> Checkpoint {
        match self {
            Writer::Json(w) => Checkpoint::Json(w.checkpoint()),
        }
    }

    /// Undo everything written since `cp` (drop a non-matching union member's
    /// partial output, or an omitted None value together with its key).
    #[inline(always)]
    pub(crate) fn rollback(&mut self, cp: Checkpoint) {
        match (self, cp) {
            (Writer::Json(w), Checkpoint::Json(c)) => w.rollback(c),
        }
    }

    /// Start position of the next value written — pair with `tail_is_null`.
    #[inline(always)]
    pub(crate) fn position(&self) -> usize {
        match self {
            Writer::Json(w) => w.position(),
        }
    }

    /// Whether the value written since `from` encodes null. Lets omit_none decide
    /// format-agnostically, keeping each format's null representation in its writer.
    #[inline(always)]
    pub(crate) fn tail_is_null(&self, from: usize) -> bool {
        match self {
            Writer::Json(w) => w.tail_is_null(from),
        }
    }

    #[inline]
    pub(crate) fn write_null(&mut self) {
        match self {
            Writer::Json(w) => w.write_null(),
        }
    }

    #[inline]
    pub(crate) fn write_bool(&mut self, v: bool) {
        match self {
            Writer::Json(w) => w.write_bool(v),
        }
    }

    #[inline]
    pub(crate) fn write_i64(&mut self, v: i64) {
        match self {
            Writer::Json(w) => w.write_i64(v),
        }
    }

    /// Integer beyond i64 — decimal string (str(int) on the Python side).
    #[inline]
    pub(crate) fn write_big_int(&mut self, v: &str) {
        match self {
            Writer::Json(w) => w.write_raw_number(v),
        }
    }

    /// Err — when the format cannot represent the value (JSON: NaN/Infinity).
    #[inline]
    pub(crate) fn write_f64(&mut self, v: f64) -> Result<(), &'static str> {
        match self {
            Writer::Json(w) => w.write_f64(v),
        }
    }

    #[inline]
    pub(crate) fn write_str(&mut self, v: &str) {
        match self {
            Writer::Json(w) => w.write_str(v),
        }
    }

    #[inline]
    pub(crate) fn begin_map(&mut self) {
        match self {
            Writer::Json(w) => w.begin_map(),
        }
    }

    #[inline]
    pub(crate) fn map_key(&mut self, key: &str) {
        match self {
            Writer::Json(w) => w.map_key(key),
        }
    }

    /// Write a key pre-rendered by [`EncodedKey`] — no escaping, one copy.
    #[inline]
    pub(crate) fn map_key_encoded(&mut self, key: &EncodedKey) {
        match self {
            Writer::Json(w) => w.map_key_encoded(&key.json),
        }
    }

    #[inline]
    pub(crate) fn end_map(&mut self) {
        match self {
            Writer::Json(w) => w.end_map(),
        }
    }

    #[inline]
    pub(crate) fn begin_array(&mut self) {
        match self {
            Writer::Json(w) => w.begin_array(),
        }
    }

    /// Call before EACH array element (JSON: commas).
    #[inline]
    pub(crate) fn array_item(&mut self) {
        match self {
            Writer::Json(w) => w.array_item(),
        }
    }

    #[inline]
    pub(crate) fn end_array(&mut self) {
        match self {
            Writer::Json(w) => w.end_array(),
        }
    }
}

#[derive(Debug)]
pub(crate) enum Parser<'j> {
    Json(JsonParser<'j>),
}

impl<'j> Parser<'j> {
    pub(crate) fn new(format: u8, data: &'j [u8]) -> Result<Self, PyErr> {
        match format {
            FORMAT_JSON => Ok(Parser::Json(JsonParser::new(data))),
            _ => Err(PyValueError::new_err(format!(
                "unknown format id: {format}"
            ))),
        }
    }

    /// Sub-parser of the same format over a slice (union/discriminator re-parse).
    pub(crate) fn sub_parser(&self, data: &'j [u8]) -> Parser<'j> {
        match self {
            Parser::Json(_) => Parser::Json(JsonParser::new(data)),
        }
    }

    #[inline]
    pub(crate) fn peek(&mut self) -> Result<Kind, SerdeError> {
        match self {
            Parser::Json(p) => p.peek(),
        }
    }

    /// Take `null`, already positioned by a preceding `peek()`.
    #[inline]
    pub(crate) fn take_null_known(&mut self) -> Result<(), SerdeError> {
        match self {
            Parser::Json(p) => p.take_null_known(),
        }
    }

    /// Take a bool, already positioned + peeked by a preceding `peek()`.
    #[inline]
    pub(crate) fn take_bool_known(&mut self) -> Result<bool, SerdeError> {
        match self {
            Parser::Json(p) => p.take_bool_known(),
        }
    }

    /// Take an integer, already positioned + peeked by a preceding `peek()`.
    #[inline]
    pub(crate) fn take_int_known(&mut self) -> Result<ParsedInt, SerdeError> {
        match self {
            Parser::Json(p) => p.take_int_known(),
        }
    }

    /// Raw text of a number, already positioned + peeked by a preceding `peek()`.
    #[inline]
    pub(crate) fn take_number_str_known(&mut self) -> Result<&str, SerdeError> {
        match self {
            Parser::Json(p) => p.take_number_str_known(),
        }
    }

    /// Take a string, already positioned by a preceding `peek()`.
    #[inline]
    pub(crate) fn take_str_known(&mut self) -> Result<&str, SerdeError> {
        match self {
            Parser::Json(p) => p.take_str_known(),
        }
    }

    /// Enter an object, already positioned + peeked by a preceding `peek()`.
    /// None — empty object; Some(key) — first key.
    #[inline]
    pub(crate) fn enter_map_known(&mut self) -> Result<Option<&str>, SerdeError> {
        match self {
            Parser::Json(p) => p.enter_map_known(),
        }
    }

    /// Enter an array, already positioned + peeked by a preceding `peek()`.
    /// true — has a first element; false — empty array.
    #[inline]
    pub(crate) fn enter_array_known(&mut self) -> Result<bool, SerdeError> {
        match self {
            Parser::Json(p) => p.enter_array_known(),
        }
    }

    /// Enter an object, self-positioning (peeks internally). For a caller that
    /// reads an object without a preceding `peek()` (discriminated-union scan).
    /// None — the object is empty; Some(key) — the first key.
    #[inline]
    pub(crate) fn enter_map(&mut self) -> Result<Option<&str>, SerdeError> {
        match self {
            Parser::Json(p) => p.enter_map(),
        }
    }

    /// Next key of the current object (None — end of object).
    #[inline]
    pub(crate) fn next_key(&mut self) -> Result<Option<&str>, SerdeError> {
        match self {
            Parser::Json(p) => p.next_key(),
        }
    }

    /// true — there is a next element; false — end of array.
    #[inline]
    pub(crate) fn next_array_item(&mut self) -> Result<bool, SerdeError> {
        match self {
            Parser::Json(p) => p.next_array_item(),
        }
    }

    #[inline]
    pub(crate) fn skip_value(&mut self) -> Result<(), SerdeError> {
        match self {
            Parser::Json(p) => p.skip_value(),
        }
    }

    /// Skip the whole value and return its raw slice (union re-parse).
    #[inline]
    pub(crate) fn take_raw_value(&mut self) -> Result<&'j [u8], SerdeError> {
        match self {
            Parser::Json(p) => p.take_raw_value(),
        }
    }

    /// Ensure input is fully consumed (trailing garbage → DecodeError).
    #[inline]
    pub(crate) fn finish(&mut self) -> Result<(), SerdeError> {
        match self {
            Parser::Json(p) => p.finish(),
        }
    }
}
