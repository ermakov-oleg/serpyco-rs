pub(crate) mod bridge;
pub(crate) mod json;

use pyo3::exceptions::PyValueError;
use pyo3::PyErr;

use json::parser::{JsonParser, ParsedInt};
use json::writer::JsonWriter;

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

impl Kind {
    // Used by direct-path encoders in later tasks for type-mismatch messages.
    #[allow(dead_code)]
    pub(crate) fn as_str(&self) -> &'static str {
        match self {
            Kind::Null => "null",
            Kind::Bool => "boolean",
            Kind::Num => "number",
            Kind::Str => "string",
            Kind::Array => "array",
            Kind::Map => "object",
        }
    }
}

pub(crate) const FORMAT_JSON: u8 = 0;

/// Enum dispatch instead of dyn: closed set of formats;
/// a new format is a new variant plus method implementations.
#[derive(Debug)]
pub(crate) enum Writer {
    Json(JsonWriter),
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

/// Enum dispatch instead of dyn: closed set of formats;
/// a new format is a new variant plus method implementations.
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
    #[allow(dead_code)]
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

    #[inline]
    pub(crate) fn take_null(&mut self) -> Result<(), SerdeError> {
        match self {
            Parser::Json(p) => p.take_null(),
        }
    }

    #[inline]
    pub(crate) fn take_bool(&mut self) -> Result<bool, SerdeError> {
        match self {
            Parser::Json(p) => p.take_bool(),
        }
    }

    #[inline]
    #[allow(dead_code)]
    pub(crate) fn take_int(&mut self) -> Result<ParsedInt, SerdeError> {
        match self {
            Parser::Json(p) => p.take_int(),
        }
    }

    #[inline]
    #[allow(dead_code)]
    pub(crate) fn take_f64(&mut self) -> Result<f64, SerdeError> {
        match self {
            Parser::Json(p) => p.take_f64(),
        }
    }

    /// Raw text of a number (for Decimal — no precision loss).
    #[inline]
    pub(crate) fn take_number_str(&mut self) -> Result<&str, SerdeError> {
        match self {
            Parser::Json(p) => p.take_number_str(),
        }
    }

    #[inline]
    pub(crate) fn take_str(&mut self) -> Result<&str, SerdeError> {
        match self {
            Parser::Json(p) => p.take_str(),
        }
    }

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

    /// true — the array has a first element; false — empty array.
    #[inline]
    pub(crate) fn enter_array(&mut self) -> Result<bool, SerdeError> {
        match self {
            Parser::Json(p) => p.enter_array(),
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
    #[allow(dead_code)]
    pub(crate) fn skip_value(&mut self) -> Result<(), SerdeError> {
        match self {
            Parser::Json(p) => p.skip_value(),
        }
    }

    /// Skip the whole value and return its raw slice (union re-parse).
    #[inline]
    #[allow(dead_code)]
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
