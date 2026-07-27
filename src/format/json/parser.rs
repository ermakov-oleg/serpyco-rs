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

/// Pull-parser over jiter. Syntax errors map to DecodeError; schema mismatches
/// are the encoders' job (peek + take_*). `*_known` readers reuse the preceding
/// `peek()`, so each is valid ONLY immediately after a `peek()` — any
/// cursor-moving call in between makes the cached peek stale. `enter_map`
/// self-positions (peeks internally) for the one caller that reads an object
/// without a preceding `peek()` (discriminated-union scan).
#[derive(Debug)]
pub(crate) struct JsonParser<'j> {
    jiter: Jiter<'j>,
    // Backing buffer for `take_raw_value` (union re-parse).
    data: &'j [u8],
    last_peek: Peek,
}

impl<'j> JsonParser<'j> {
    pub(crate) fn new(data: &'j [u8]) -> Self {
        JsonParser {
            jiter: Jiter::new(data),
            data,
            last_peek: Peek::Null,
        }
    }

    #[inline]
    pub(crate) fn peek(&mut self) -> Result<Kind, SerdeError> {
        let peek = self.jiter.peek().map_err(err)?;
        self.last_peek = peek;
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
    pub(crate) fn take_null_known(&mut self) -> Result<(), SerdeError> {
        self.jiter.known_null().map_err(err)
    }

    #[inline]
    pub(crate) fn take_bool_known(&mut self) -> Result<bool, SerdeError> {
        self.jiter.known_bool(self.last_peek).map_err(err)
    }

    /// jiter emits a "found float" error for a float-shaped token (e.g. `1.5`);
    /// the caller maps that to the schema-level "not an integer" error.
    #[inline]
    pub(crate) fn take_int_known(&mut self) -> Result<ParsedInt, SerdeError> {
        match self.jiter.known_int(self.last_peek).map_err(err)? {
            NumberInt::Int(v) => Ok(ParsedInt::I64(v)),
            NumberInt::BigInt(v) => Ok(ParsedInt::Big(v)),
        }
    }

    /// Raw text of a number, so Decimal and the int-vs-float split lose no precision.
    #[inline]
    pub(crate) fn take_number_str_known(&mut self) -> Result<&str, SerdeError> {
        let bytes = self.jiter.known_number_bytes(self.last_peek).map_err(err)?;
        // jiter guarantees ASCII digits/sign/dot/exponent here
        Ok(unsafe { std::str::from_utf8_unchecked(bytes) })
    }

    #[inline]
    pub(crate) fn take_str_known(&mut self) -> Result<&str, SerdeError> {
        self.jiter.known_str().map_err(err)
    }

    /// None — empty object; Some(key) — first key.
    #[inline]
    pub(crate) fn enter_map_known(&mut self) -> Result<Option<&str>, SerdeError> {
        self.jiter.known_object().map_err(err)
    }

    /// true — has a first element; false — empty array.
    #[inline]
    pub(crate) fn enter_array_known(&mut self) -> Result<bool, SerdeError> {
        Ok(self.jiter.known_array().map_err(err)?.is_some())
    }

    /// Enter an object without a preceding `peek()` (discriminated-union scan).
    #[inline]
    pub(crate) fn enter_map(&mut self) -> Result<Option<&str>, SerdeError> {
        self.jiter.next_object().map_err(err)
    }

    /// Next key of the current object (None — end of object).
    #[inline]
    pub(crate) fn next_key(&mut self) -> Result<Option<&str>, SerdeError> {
        self.jiter.next_key().map_err(err)
    }

    /// true — there is a next element; false — end of array.
    #[inline]
    pub(crate) fn next_array_item(&mut self) -> Result<bool, SerdeError> {
        Ok(self.jiter.array_step().map_err(err)?.is_some())
    }

    /// Bounded by jiter's own recursion limit (~200), not `max_recursion_depth`:
    /// a deeply nested value under an unknown key fails with "recursion limit
    /// exceeded", while the same value on an `Any` field goes through `parse_any`
    /// and honours `max_recursion_depth`.
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

/// Takes `JiterError` by value rather than being a `&self` method: jiter's
/// readers return `&str` tied to `&mut self.jiter`, which a `|e| self.err(e)`
/// closure's borrow would conflict with.
#[inline]
fn err(e: JiterError) -> SerdeError {
    SerdeError::Py(decode_err(&e))
}

// Built from `error_type` alone, not `JiterError`'s Display: that already appends
// "at index N", and `DecodeError.__str__` appends "(position N)" itself.
#[inline]
pub(crate) fn decode_err(e: &JiterError) -> PyErr {
    DecodeError::new_err((format!("{}", e.error_type), e.index))
}
