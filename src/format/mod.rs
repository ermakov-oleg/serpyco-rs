pub(crate) mod bridge;
pub(crate) mod json;
pub(crate) mod msgpack;

use num_bigint::BigInt;
use pyo3::exceptions::PyValueError;
use pyo3::PyErr;

use json::parser::JsonParser;
use json::writer::{Checkpoint as JsonCheckpoint, JsonWriter};
use msgpack::parser::MsgpackParser;
use msgpack::writer::{Checkpoint as MsgpackCheckpoint, MsgpackWriter};

use crate::serde_error::SerdeError;

/// Kind of the next value in the input stream (format-agnostic).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Kind {
    Null,
    Bool,
    Num,
    Str,
    Bytes,
    Array,
    Map,
}

pub(crate) const FORMAT_JSON: u8 = 0;
pub(crate) const FORMAT_MSGPACK: u8 = 1;

/// Integer from a wire format. MessagePack is capped at u64/i64, while JSON can
/// carry arbitrary-length integer tokens.
pub(crate) enum ParsedInt {
    I64(i64),
    Big(BigInt),
}

/// Cap on a wire value rendered into a schema-error message. Error paths must
/// not amplify attacker-controlled input: a huge value at the mismatch point
/// contributes at most this many bytes to the message, never a copy of the
/// whole payload.
pub(crate) const VALUE_REPR_LIMIT: usize = 2048;

/// `String::from_utf8_lossy` capped at [`VALUE_REPR_LIMIT`] (an ellipsis marks
/// the cut; a UTF-8 sequence split by the cut lossily degrades — error text only).
pub(crate) fn lossy_repr_truncated(data: &[u8]) -> String {
    if data.len() <= VALUE_REPR_LIMIT {
        String::from_utf8_lossy(data).into_owned()
    } else {
        let mut out = String::from_utf8_lossy(&data[..VALUE_REPR_LIMIT]).into_owned();
        out.push('…');
        out
    }
}

/// Any number from the wire, already split int-vs-float by the format itself:
/// JSON by token shape (dot/exponent), MessagePack by marker. Callers that need
/// the exact wire text (Decimal) use `take_number_str_known` instead.
pub(crate) enum ParsedNumber {
    Int(ParsedInt),
    F64(f64),
}

/// Enum dispatch instead of dyn: the set of formats is closed.
// large_enum_variant: MsgpackWriter carries an inline container stack. The
// Writer is built once per dump and stays on that call's stack — boxing the
// variant would trade a one-time size cost for pointer indirection on every
// write call in the hot path.
#[allow(clippy::large_enum_variant)]
#[derive(Debug)]
pub(crate) enum Writer {
    Json(JsonWriter),
    Msgpack(MsgpackWriter),
}

/// Opaque saved writer position, paired with `checkpoint` / `rollback`.
#[derive(Debug, Clone, Copy)]
pub(crate) enum Checkpoint {
    Json(JsonCheckpoint),
    Msgpack(MsgpackCheckpoint),
}

/// A map key rendered once at encoder-construction time: Entity/TypedDict keys
/// are fixed by the type, so the hot path copies these bytes instead of re-escaping.
#[derive(Debug, Clone)]
pub(crate) struct EncodedKey {
    json: Box<[u8]>,
    msgpack: Box<[u8]>,
}

impl EncodedKey {
    pub(crate) fn new(key: &str) -> Self {
        EncodedKey {
            json: json::writer::encode_map_key(key),
            msgpack: msgpack::writer::encode_map_key(key),
        }
    }
}

impl Writer {
    /// `capacity` is a size hint from the previous dump of the same serializer;
    /// it only affects how often the buffer grows.
    pub(crate) fn with_capacity(format: u8, capacity: usize) -> Result<Self, PyErr> {
        match format {
            FORMAT_JSON => Ok(Writer::Json(JsonWriter::with_capacity(capacity))),
            FORMAT_MSGPACK => Ok(Writer::Msgpack(MsgpackWriter::with_capacity(capacity))),
            _ => Err(PyValueError::new_err(format!(
                "unknown format id: {format}"
            ))),
        }
    }

    #[inline]
    pub(crate) fn as_bytes(&self) -> &[u8] {
        match self {
            Writer::Json(w) => w.as_bytes(),
            Writer::Msgpack(w) => w.as_bytes(),
        }
    }

    /// Snapshot the writer so a speculative value (union member probe, omit_none
    /// value) can be rolled back in place.
    #[inline(always)]
    pub(crate) fn checkpoint(&self) -> Checkpoint {
        match self {
            Writer::Json(w) => Checkpoint::Json(w.checkpoint()),
            Writer::Msgpack(w) => Checkpoint::Msgpack(w.checkpoint()),
        }
    }

    #[inline(always)]
    pub(crate) fn rollback(&mut self, cp: Checkpoint) {
        match (self, cp) {
            (Writer::Json(w), Checkpoint::Json(c)) => w.rollback(c),
            (Writer::Msgpack(w), Checkpoint::Msgpack(c)) => w.rollback(c),
            _ => unreachable!("checkpoint belongs to a different format"),
        }
    }

    /// Start position of the next value written — pair with `tail_is_null`.
    #[inline(always)]
    pub(crate) fn position(&self) -> usize {
        match self {
            Writer::Json(w) => w.position(),
            Writer::Msgpack(w) => w.position(),
        }
    }

    /// Whether the value written since `from` encodes null. Lets omit_none decide
    /// format-agnostically, each format keeping its own null representation.
    #[inline(always)]
    pub(crate) fn tail_is_null(&self, from: usize) -> bool {
        match self {
            Writer::Json(w) => w.tail_is_null(from),
            Writer::Msgpack(w) => w.tail_is_null(from),
        }
    }

    #[inline]
    pub(crate) fn write_null(&mut self) {
        match self {
            Writer::Json(w) => w.write_null(),
            Writer::Msgpack(w) => w.write_null(),
        }
    }

    #[inline]
    pub(crate) fn write_bool(&mut self, v: bool) {
        match self {
            Writer::Json(w) => w.write_bool(v),
            Writer::Msgpack(w) => w.write_bool(v),
        }
    }

    #[inline]
    pub(crate) fn write_i64(&mut self, v: i64) {
        match self {
            Writer::Json(w) => w.write_i64(v),
            Writer::Msgpack(w) => w.write_i64(v),
        }
    }

    /// Integer beyond i64 — decimal string (`str(int)` on the Python side).
    #[inline]
    pub(crate) fn write_big_int(&mut self, v: &str) -> Result<(), &'static str> {
        match self {
            Writer::Json(w) => {
                w.write_raw_number(v);
                Ok(())
            }
            Writer::Msgpack(w) => w.write_big_int(v),
        }
    }

    /// Err — when the format cannot represent the value (JSON: NaN/Infinity).
    #[inline]
    pub(crate) fn write_f64(&mut self, v: f64) -> Result<(), &'static str> {
        match self {
            Writer::Json(w) => w.write_f64(v),
            Writer::Msgpack(w) => w.write_f64(v),
        }
    }

    #[inline]
    pub(crate) fn write_str(&mut self, v: &str) {
        match self {
            Writer::Json(w) => w.write_str(v),
            Writer::Msgpack(w) => w.write_str(v),
        }
    }

    #[inline]
    pub(crate) fn write_bytes(&mut self, v: &[u8]) -> Result<(), &'static str> {
        match self {
            Writer::Json(_) => Err("bytes values are not supported by this format"),
            Writer::Msgpack(w) => {
                w.write_bytes(v);
                Ok(())
            }
        }
    }

    /// `len` — number of entries when the caller knows it up front (`None` when
    /// it depends on what gets written, e.g. omit_none). JSON ignores it; sized
    /// binary formats use it to emit an exact header instead of backpatching.
    #[inline]
    pub(crate) fn begin_map(&mut self, len: Option<usize>) {
        match self {
            Writer::Json(w) => w.begin_map(),
            Writer::Msgpack(w) => w.begin_map(len),
        }
    }

    #[inline]
    pub(crate) fn map_key(&mut self, key: &str) {
        match self {
            Writer::Json(w) => w.map_key(key),
            Writer::Msgpack(w) => w.map_key(key),
        }
    }

    /// Write a key pre-rendered by [`EncodedKey`] — no escaping, one copy.
    #[inline]
    pub(crate) fn map_key_encoded(&mut self, key: &EncodedKey) {
        match self {
            Writer::Json(w) => w.map_key_encoded(&key.json),
            Writer::Msgpack(w) => w.map_key_encoded(&key.msgpack),
        }
    }

    #[inline]
    pub(crate) fn end_map(&mut self) {
        match self {
            Writer::Json(w) => w.end_map(),
            Writer::Msgpack(w) => w.end_map(),
        }
    }

    /// See [`Writer::begin_map`] for the `len` contract.
    #[inline]
    pub(crate) fn begin_array(&mut self, len: Option<usize>) {
        match self {
            Writer::Json(w) => w.begin_array(),
            Writer::Msgpack(w) => w.begin_array(len),
        }
    }

    /// Call after each array element and after each map value.
    #[inline]
    pub(crate) fn item_end(&mut self) {
        match self {
            Writer::Json(w) => w.item_end(),
            Writer::Msgpack(w) => w.item_end(),
        }
    }

    #[inline]
    pub(crate) fn end_array(&mut self) {
        match self {
            Writer::Json(w) => w.end_array(),
            Writer::Msgpack(w) => w.end_array(),
        }
    }
}

/// Pull-parser over the input. `*_known` readers require an immediately preceding
/// `peek()` (any cursor-moving call between invalidates it); `enter_map` peeks internally.
#[derive(Debug)]
pub(crate) enum Parser<'j> {
    Json(JsonParser<'j>),
    Msgpack(MsgpackParser<'j>),
}

impl<'j> Parser<'j> {
    pub(crate) fn new(format: u8, data: &'j [u8]) -> Result<Self, PyErr> {
        match format {
            FORMAT_JSON => Ok(Parser::Json(JsonParser::new(data))),
            FORMAT_MSGPACK => Ok(Parser::Msgpack(MsgpackParser::new(data))),
            _ => Err(PyValueError::new_err(format!(
                "unknown format id: {format}"
            ))),
        }
    }

    /// Sub-parser of the same format over a slice (union/discriminator re-parse).
    pub(crate) fn sub_parser(&self, data: &'j [u8]) -> Parser<'j> {
        match self {
            Parser::Json(_) => Parser::Json(JsonParser::new(data)),
            Parser::Msgpack(_) => Parser::Msgpack(MsgpackParser::new(data)),
        }
    }

    #[inline(always)]
    pub(crate) fn peek(&mut self) -> Result<Kind, SerdeError> {
        match self {
            Parser::Json(p) => p.peek(),
            Parser::Msgpack(p) => p.peek(),
        }
    }

    #[inline(always)]
    pub(crate) fn take_null_known(&mut self) -> Result<(), SerdeError> {
        match self {
            Parser::Json(p) => p.take_null_known(),
            Parser::Msgpack(p) => p.take_null_known(),
        }
    }

    #[inline(always)]
    pub(crate) fn take_bool_known(&mut self) -> Result<bool, SerdeError> {
        match self {
            Parser::Json(p) => p.take_bool_known(),
            Parser::Msgpack(p) => p.take_bool_known(),
        }
    }

    #[inline(always)]
    pub(crate) fn take_int_known(&mut self) -> Result<ParsedInt, SerdeError> {
        match self {
            Parser::Json(p) => p.take_int_known(),
            Parser::Msgpack(p) => p.take_int_known(),
        }
    }

    /// Typed number (int-vs-float decided by the format) — the hot path for
    /// float/Any loads; binary formats read it without going through text.
    #[inline(always)]
    pub(crate) fn take_number_known(&mut self) -> Result<ParsedNumber, SerdeError> {
        match self {
            Parser::Json(p) => p.take_number_known(),
            Parser::Msgpack(p) => p.take_number_known(),
        }
    }

    /// Raw text of a number (Decimal, error rendering).
    #[inline]
    pub(crate) fn take_number_str_known(&mut self) -> Result<&str, SerdeError> {
        match self {
            Parser::Json(p) => p.take_number_str_known(),
            Parser::Msgpack(p) => p.take_number_str_known(),
        }
    }

    #[inline(always)]
    pub(crate) fn take_str_known(&mut self) -> Result<&str, SerdeError> {
        match self {
            Parser::Json(p) => p.take_str_known(),
            Parser::Msgpack(p) => p.take_str_known(),
        }
    }

    /// Materialize the next string as a `PyString` (hot-path alternative to
    /// `take_str_known` + `PyString::new` when the value goes straight to
    /// Python). MessagePack reuses its validation's ASCII knowledge; JSON
    /// applies the same ASCII fast path over jiter's already-validated str.
    #[inline(always)]
    pub(crate) fn take_pystring_known<'py>(
        &mut self,
        py: pyo3::Python<'py>,
    ) -> Result<pyo3::Bound<'py, pyo3::types::PyString>, SerdeError> {
        match self {
            Parser::Json(p) => Ok(crate::python::create_py_string(py, p.take_str_known()?)?),
            Parser::Msgpack(p) => p.take_pystring_known(py),
        }
    }

    #[inline(always)]
    pub(crate) fn take_bytes_known(&mut self) -> Result<&'j [u8], SerdeError> {
        match self {
            Parser::Json(_) => unreachable!("JSON never reports Kind::Bytes"),
            Parser::Msgpack(p) => p.take_bytes_known(),
        }
    }

    /// None — empty object; Some(key) — first key.
    #[inline(always)]
    pub(crate) fn enter_map_known(&mut self) -> Result<Option<&str>, SerdeError> {
        match self {
            Parser::Json(p) => p.enter_map_known(),
            Parser::Msgpack(p) => p.enter_map_known(),
        }
    }

    /// true — has a first element; false — empty array.
    #[inline(always)]
    pub(crate) fn enter_array_known(&mut self) -> Result<bool, SerdeError> {
        match self {
            Parser::Json(p) => p.enter_array_known(),
            Parser::Msgpack(p) => p.enter_array_known(),
        }
    }

    /// Entry count of the container just entered, when the format states it up front:
    /// MessagePack headers do, JSON only reveals it at the closing bracket. Lets a
    /// caller size its Python container once instead of regrowing it per entry.
    #[inline(always)]
    pub(crate) fn container_len_hint(&self) -> Option<usize> {
        match self {
            Parser::Json(_) => None,
            Parser::Msgpack(p) => p.container_len_hint(),
        }
    }

    /// [`enter_map_known`](Self::enter_map_known) paired with the entry count from
    /// [`container_len_hint`](Self::container_len_hint). Both come out of one call
    /// because the borrowed first key rules out a second one.
    #[inline(always)]
    pub(crate) fn enter_map_known_sized(
        &mut self,
    ) -> Result<(Option<&str>, Option<usize>), SerdeError> {
        match self {
            Parser::Json(p) => Ok((p.enter_map_known()?, None)),
            Parser::Msgpack(p) => {
                let (key, len) = p.enter_map_known_sized()?;
                Ok((key, Some(len)))
            }
        }
    }

    /// Enter an object without a preceding `peek()` (discriminated-union scan).
    #[inline]
    pub(crate) fn enter_map(&mut self) -> Result<Option<&str>, SerdeError> {
        match self {
            Parser::Json(p) => p.enter_map(),
            Parser::Msgpack(p) => p.enter_map(),
        }
    }

    /// Next key of the current object (None — end of object).
    #[inline(always)]
    pub(crate) fn next_key(&mut self) -> Result<Option<&str>, SerdeError> {
        match self {
            Parser::Json(p) => p.next_key(),
            Parser::Msgpack(p) => p.next_key(),
        }
    }

    /// true — there is a next element; false — end of array.
    #[inline(always)]
    pub(crate) fn next_array_item(&mut self) -> Result<bool, SerdeError> {
        match self {
            Parser::Json(p) => p.next_array_item(),
            Parser::Msgpack(p) => p.next_array_item(),
        }
    }

    #[inline]
    pub(crate) fn skip_value(&mut self) -> Result<(), SerdeError> {
        match self {
            Parser::Json(p) => p.skip_value(),
            Parser::Msgpack(p) => p.skip_value(),
        }
    }

    /// Skip the whole value and return its raw slice (union re-parse).
    #[inline]
    pub(crate) fn take_raw_value(&mut self) -> Result<&'j [u8], SerdeError> {
        match self {
            Parser::Json(p) => p.take_raw_value(),
            Parser::Msgpack(p) => p.take_raw_value(),
        }
    }

    /// Consume one value and render it for a schema error. JSON keeps its raw
    /// wire text; MessagePack reconstructs the equivalent Python-style value.
    /// Both renderings are capped at [`VALUE_REPR_LIMIT`].
    #[inline]
    pub(crate) fn take_value_repr(&mut self) -> Result<String, SerdeError> {
        match self {
            Parser::Json(p) => Ok(lossy_repr_truncated(p.take_raw_value()?)),
            Parser::Msgpack(p) => p.take_value_repr(),
        }
    }

    /// Render an already captured value span without affecting this parser.
    #[inline]
    pub(crate) fn value_repr(&self, data: &'j [u8]) -> Result<String, SerdeError> {
        match self {
            Parser::Json(_) => Ok(lossy_repr_truncated(data)),
            Parser::Msgpack(_) => MsgpackParser::new(data).take_value_repr(),
        }
    }

    /// Ensure input is fully consumed (trailing garbage → DecodeError).
    #[inline]
    pub(crate) fn finish(&mut self) -> Result<(), SerdeError> {
        match self {
            Parser::Json(p) => p.finish(),
            Parser::Msgpack(p) => p.finish(),
        }
    }
}
