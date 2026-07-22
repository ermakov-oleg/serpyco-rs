// pub(crate) mod bridge; // will be added in Task 3 — keep commented
pub(crate) mod json;

use pyo3::exceptions::PyValueError;
use pyo3::PyErr;

use json::writer::JsonWriter;

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
