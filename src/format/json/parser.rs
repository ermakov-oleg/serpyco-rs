use jiter::{Jiter, JiterError, NumberInt, Peek};
use num_bigint::BigInt;
use pyo3::PyErr;

use crate::errors::{DecodeError, ToPyErr};
use crate::format::Kind;
use crate::serde_error::SerdeError;

/// Integer from the stream: JSON allows arbitrary-length numbers.
pub(crate) enum ParsedInt {
    I64(i64),
    Big(BigInt),
}

/// Pull-parser over jiter. All jiter (syntax) errors map to DecodeError with
/// a byte position; schema mismatches are NOT the parser's job — encoders
/// decide via peek + take_*.
///
/// Every `take_*` method is self-positioning: it peeks internally first (peek
/// eats leading whitespace and lands the cursor on the value byte), so
/// encoders may call `take_*` directly after `next_key`/`enter_array` without
/// a preceding peek of their own.
#[derive(Debug)]
pub(crate) struct JsonParser<'j> {
    jiter: Jiter<'j>,
    data: &'j [u8],
}

impl<'j> JsonParser<'j> {
    pub(crate) fn new(data: &'j [u8]) -> Self {
        JsonParser {
            jiter: Jiter::new(data),
            data,
        }
    }

    #[inline]
    pub(crate) fn peek(&mut self) -> Result<Kind, SerdeError> {
        let peek = self.jiter.peek().map_err(err)?;
        Ok(match peek {
            Peek::Null => Kind::Null,
            Peek::True | Peek::False => Kind::Bool,
            Peek::String => Kind::Str,
            Peek::Array => Kind::Array,
            Peek::Object => Kind::Map,
            _ => Kind::Num, // digits and '-'
        })
    }

    #[inline]
    pub(crate) fn take_null(&mut self) -> Result<(), SerdeError> {
        // known_null() doesn't eat whitespace on its own — peek first to
        // land the cursor on the value byte (e.g. after a `next_key` colon).
        self.jiter.peek().map_err(err)?;
        self.jiter.known_null().map_err(err)
    }

    #[inline]
    pub(crate) fn take_bool(&mut self) -> Result<bool, SerdeError> {
        let peek = self.jiter.peek().map_err(err)?;
        self.jiter.known_bool(peek).map_err(err)
    }

    #[inline]
    pub(crate) fn take_int(&mut self) -> Result<ParsedInt, SerdeError> {
        let peek = self.jiter.peek().map_err(err)?;
        match self.jiter.known_int(peek).map_err(err)? {
            NumberInt::Int(v) => Ok(ParsedInt::I64(v)),
            NumberInt::BigInt(v) => Ok(ParsedInt::Big(v)),
        }
    }

    #[inline]
    pub(crate) fn take_f64(&mut self) -> Result<f64, SerdeError> {
        let peek = self.jiter.peek().map_err(err)?;
        self.jiter.known_float(peek).map_err(err)
    }

    /// Raw text of a number (for Decimal — no precision loss).
    #[inline]
    pub(crate) fn take_number_str(&mut self) -> Result<&str, SerdeError> {
        let bytes = self.jiter.next_number_bytes().map_err(err)?;
        // jiter guarantees ASCII digits/sign/dot/exponent here
        Ok(unsafe { std::str::from_utf8_unchecked(bytes) })
    }

    #[inline]
    pub(crate) fn take_str(&mut self) -> Result<&str, SerdeError> {
        // known_str() doesn't eat whitespace on its own — peek first to
        // land the cursor on the value byte (e.g. after a `next_key` colon).
        self.jiter.peek().map_err(err)?;
        self.jiter.known_str().map_err(err)
    }

    /// None — the object is empty; Some(key) — the first key.
    #[inline]
    pub(crate) fn enter_map(&mut self) -> Result<Option<&str>, SerdeError> {
        self.jiter.next_object().map_err(err)
    }

    /// Next key of the current object (None — end of object).
    #[inline]
    pub(crate) fn next_key(&mut self) -> Result<Option<&str>, SerdeError> {
        self.jiter.next_key().map_err(err)
    }

    /// true — the array has a first element; false — empty array.
    #[inline]
    pub(crate) fn enter_array(&mut self) -> Result<bool, SerdeError> {
        Ok(self.jiter.next_array().map_err(err)?.is_some())
    }

    /// true — there is a next element; false — end of array.
    #[inline]
    pub(crate) fn next_array_item(&mut self) -> Result<bool, SerdeError> {
        Ok(self.jiter.array_step().map_err(err)?.is_some())
    }

    #[inline]
    pub(crate) fn skip_value(&mut self) -> Result<(), SerdeError> {
        self.jiter.next_skip().map_err(err)
    }

    /// Skip the whole value and return its raw slice (union re-parse).
    #[inline]
    pub(crate) fn take_raw_value(&mut self) -> Result<&'j [u8], SerdeError> {
        // peek consumes whitespace; current_index lands on the value start
        self.jiter.peek().map_err(err)?;
        let start = self.jiter.current_index();
        self.jiter.next_skip().map_err(err)?;
        let end = self.jiter.current_index();
        Ok(&self.data[start..end])
    }

    /// Ensure input is fully consumed (trailing garbage → DecodeError).
    #[inline]
    pub(crate) fn finish(&mut self) -> Result<(), SerdeError> {
        self.jiter.finish().map_err(err)
    }
}

/// Map a jiter syntax error to our `SerdeError`. Takes `JiterError` by value
/// (a free function, not a `&self` method) so it never re-borrows `self` —
/// several jiter methods return `&str`/`Option<&str>` tied to `&mut self.jiter`,
/// and a closure like `|e| self.err(e)` would conflict with that live borrow.
/// The byte offset comes straight from `JiterError::index`, which jiter
/// records at the exact point of failure.
#[inline]
fn err(e: JiterError) -> SerdeError {
    SerdeError::Py(decode_err(&e))
}

// Build the message from `error_type` alone (not `JiterError`'s own Display,
// which already appends "at index N") — `DecodeError.__str__` appends
// "(position N)" itself, so using `{e}` here would double the offset.
#[inline]
pub(crate) fn decode_err(e: &JiterError) -> PyErr {
    DecodeError::new_err((format!("{}", e.error_type), e.index))
}
