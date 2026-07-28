use jiter::{Jiter, JiterError, JiterErrorType, JsonErrorType, NumberInt, Peek};
use pyo3::PyErr;

use crate::errors::{DecodeError, ToPyErr};
use crate::format::{Kind, ParsedInt, ParsedNumber};
use crate::serde_error::SerdeError;

/// Pull-parser over jiter. Syntax errors map to `DecodeError`; schema mismatches are
/// the encoders' job. `*_known` readers are valid only immediately after a `peek()`;
/// `enter_map` peeks internally for the one caller without one (discriminated-union scan).
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

    // `Peek` is a newtype over the lead byte, not an enum, so match the byte itself
    // (`into_inner()`) rather than `Peek`'s consts — lets the digit run be a range
    // pattern, checked first since numbers are JSON's most frequent token. The
    // catch-all errors instead of assuming "number", so a future `Peek` kind fails
    // loudly here rather than being fed to the number reader.
    #[inline]
    pub(crate) fn peek(&mut self) -> Result<Kind, SerdeError> {
        let peek = self.jiter.peek().map_err(err)?;
        self.last_peek = peek;
        Ok(match peek.into_inner() {
            b'0'..=b'9' | b'-' => Kind::Num,
            b'n' => Kind::Null,
            b't' | b'f' => Kind::Bool,
            b'"' => Kind::Str,
            b'[' => Kind::Array,
            b'{' => Kind::Map,
            _ => return Err(unexpected_token_err(self.jiter.current_index())),
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

    /// Typed number: int-vs-float decided by token shape (dot/exponent). Goes
    /// through the raw token text — measured faster than jiter's `known_number`
    /// (std's Eisel-Lemire f64 parse), and keeps the int split byte-identical
    /// to the historical text-sniffing behavior.
    #[inline]
    pub(crate) fn take_number_known(&mut self) -> Result<ParsedNumber, SerdeError> {
        let index = self.jiter.current_index();
        let bytes = self.jiter.known_number_bytes(self.last_peek).map_err(err)?;
        // jiter guarantees ASCII digits/sign/dot/exponent here
        let raw = unsafe { std::str::from_utf8_unchecked(bytes) };
        if raw.bytes().all(|b| b.is_ascii_digit() || b == b'-') {
            if let Ok(v) = raw.parse::<i64>() {
                return Ok(ParsedNumber::Int(ParsedInt::I64(v)));
            }
            if let Ok(v) = raw.parse::<num_bigint::BigInt>() {
                return Ok(ParsedNumber::Int(ParsedInt::Big(v)));
            }
        } else if let Ok(v) = raw.parse::<f64>() {
            return Ok(ParsedNumber::F64(v));
        }
        // Unreachable: jiter only yields valid JSON number tokens.
        Err(unexpected_token_err(index))
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

    /// Bounded by jiter's own recursion limit (~200), not `max_recursion_depth`: skipping
    /// a deeply nested value under an unknown key hits jiter's limit, not ours — only
    /// `Any` fields, via `parse_any`, honour `max_recursion_depth`.
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

/// Takes `JiterError` by value rather than being a `&self` method: jiter's readers
/// return `&str` tied to `&mut self.jiter`, which a `|e| self.err(e)` closure would conflict with.
#[inline]
fn err(e: JiterError) -> SerdeError {
    SerdeError::Py(decode_err(&e))
}

/// `peek()`'s byte matched none of JSON's token-lead bytes — reports the same
/// `ExpectedSomeValue` jiter itself uses for a bad lead byte, so message and position
/// stay consistent with every other syntax error from this parser.
#[inline]
fn unexpected_token_err(index: usize) -> SerdeError {
    err(JiterError {
        error_type: JiterErrorType::JsonError(JsonErrorType::ExpectedSomeValue),
        index,
    })
}

// Built from `error_type` alone, not `JiterError`'s Display: that already appends
// "at index N", and `DecodeError.__str__` appends "(position N)" itself.
#[inline]
pub(crate) fn decode_err(e: &JiterError) -> PyErr {
    DecodeError::new_err((format!("{}", e.error_type), e.index))
}
