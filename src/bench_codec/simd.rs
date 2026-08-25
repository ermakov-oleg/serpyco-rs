//! Byte scanning for the specialized codec (benchmark only).
//!
//! Both hot loops — finding where a JSON string ends, and finding the next byte
//! that has to be escaped — are "scan until one of three byte classes appears".
//! SSE2 is part of the x86-64 baseline, so these can be `#[inline(always)]`
//! without a `#[target_feature]` gate (which would force an out-of-line call at
//! every use site). Other architectures fall back to a byte loop.

#[cfg(target_arch = "x86_64")]
use std::arch::x86_64::{
    __m128i, _mm_cmpeq_epi8, _mm_loadu_si128, _mm_max_epu8, _mm_movemask_epi8, _mm_or_si128,
    _mm_set1_epi8,
};

/// The byte classes each scan stops on.
#[derive(Clone, Copy)]
pub(super) enum Stop {
    /// End of a string token, or anything the fast path cannot handle:
    /// `"`, `\`, or a non-ASCII byte.
    StringEnd,
    /// `"` or `\` only — used once a string is known to be non-ASCII.
    QuoteOrBackslash,
    /// Bytes JSON forbids unescaped: `"`, `\`, or a control character.
    NeedsEscape,
}

#[inline(always)]
fn matches(stop: Stop, b: u8) -> bool {
    match stop {
        Stop::StringEnd => b == b'"' || b == b'\\' || b >= 0x80,
        Stop::QuoteOrBackslash => b == b'"' || b == b'\\',
        Stop::NeedsEscape => b == b'"' || b == b'\\' || b < 0x20,
    }
}

#[cfg(target_arch = "x86_64")]
#[inline(always)]
unsafe fn lane_mask(stop: Stop, v: __m128i) -> i32 {
    let quote = _mm_cmpeq_epi8(v, _mm_set1_epi8(b'"' as i8));
    let backslash = _mm_cmpeq_epi8(v, _mm_set1_epi8(b'\\' as i8));
    let both = _mm_or_si128(quote, backslash);
    match stop {
        // `movemask` of the raw vector is exactly "high bit set", i.e. >= 0x80.
        Stop::StringEnd => _mm_movemask_epi8(both) | _mm_movemask_epi8(v),
        Stop::QuoteOrBackslash => _mm_movemask_epi8(both),
        // `max_epu8(v, 0x1F) == 0x1F` is an unsigned `v <= 0x1F`.
        Stop::NeedsEscape => {
            let limit = _mm_set1_epi8(0x1F);
            let ctrl = _mm_cmpeq_epi8(_mm_max_epu8(v, limit), limit);
            _mm_movemask_epi8(_mm_or_si128(both, ctrl))
        }
    }
}

/// Index of the first byte at or after `from` that belongs to `stop`'s class.
#[inline(always)]
pub(super) fn find(stop: Stop, bytes: &[u8], from: usize) -> Option<usize> {
    let mut i = from;
    #[cfg(target_arch = "x86_64")]
    while i + 16 <= bytes.len() {
        // Safety: the 16 bytes at `i` are in bounds by the loop condition.
        let mask = unsafe { lane_mask(stop, _mm_loadu_si128(bytes.as_ptr().add(i).cast())) };
        if mask != 0 {
            return Some(i + mask.trailing_zeros() as usize);
        }
        i += 16;
    }
    while i < bytes.len() {
        if matches(stop, bytes[i]) {
            return Some(i);
        }
        i += 1;
    }
    None
}
