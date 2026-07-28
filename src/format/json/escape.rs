/// The exact set JSON requires escaping: `"`, `\` and control chars.
#[inline(always)]
fn needs_escape(b: u8) -> bool {
    b < 0x20 || b == b'"' || b == b'\\'
}

const LANE: u64 = 0x0101_0101_0101_0101;
const HIGH: u64 = 0x8080_8080_8080_8080;

/// Per-byte SWAR predicate: high bit set in every lane holding a byte that needs
/// escaping. Bytes >= 0x80 (UTF-8 continuations) never match — `!word` clears
/// their lane — so multi-byte sequences pass through untouched.
///
/// SWAR rather than a SIMD crate: a `#[target_feature(avx2)]` escape cannot be
/// inlined into callers that lack the feature, so every string paid a call.
///
/// ONLY THE LOWEST SET LANE IS TRUSTWORTHY: a borrow out of lane k can spuriously
/// set lane k+1, and such a borrow happens exactly when lane k itself matches, so
/// false positives always sit *above* a true one. Hence the caller must take one
/// byte per word and recompute.
#[inline(always)]
fn escape_lanes(word: u64) -> u64 {
    let below_0x20 = word.wrapping_sub(0x20 * LANE) & !word;
    let quote = word ^ (0x22 * LANE);
    let quote = quote.wrapping_sub(LANE) & !quote;
    let backslash = word ^ (0x5C * LANE);
    let backslash = backslash.wrapping_sub(LANE) & !backslash;
    (below_0x20 | quote | backslash) & HIGH
}

/// Write one byte in its JSON-escaped form. Matches `json.dumps`/serde_json:
/// short forms for `\b\t\n\f\r`, lowercase `\u00XX` for the other controls.
#[cold]
#[inline(never)]
fn write_escaped_byte(buf: &mut Vec<u8>, b: u8) {
    match b {
        b'"' => buf.extend_from_slice(br#"\""#),
        b'\\' => buf.extend_from_slice(br"\\"),
        0x08 => buf.extend_from_slice(br"\b"),
        0x09 => buf.extend_from_slice(br"\t"),
        0x0A => buf.extend_from_slice(br"\n"),
        0x0C => buf.extend_from_slice(br"\f"),
        0x0D => buf.extend_from_slice(br"\r"),
        _ => {
            const HEX: &[u8; 16] = b"0123456789abcdef";
            buf.extend_from_slice(&[
                b'\\',
                b'u',
                b'0',
                b'0',
                HEX[(b >> 4) as usize],
                HEX[(b & 0x0F) as usize],
            ]);
        }
    }
}

/// Append `s` JSON-escaped (without the surrounding quotes), copying clean runs
/// in bulk and escaping byte by byte only where needed.
pub(super) fn escape_into(buf: &mut Vec<u8>, s: &str) {
    let bytes = s.as_bytes();
    let mut clean_from = 0;
    let mut i = 0;

    while i + 8 <= bytes.len() {
        let word = u64::from_le_bytes(bytes[i..i + 8].try_into().expect("8-byte chunk"));
        let lanes = escape_lanes(word);
        if lanes == 0 {
            i += 8;
            continue;
        }
        let at = i + (lanes.trailing_zeros() / 8) as usize;
        buf.extend_from_slice(&bytes[clean_from..at]);
        write_escaped_byte(buf, bytes[at]);
        i = at + 1;
        clean_from = i;
    }

    while i < bytes.len() {
        let b = bytes[i];
        if needs_escape(b) {
            buf.extend_from_slice(&bytes[clean_from..i]);
            write_escaped_byte(buf, b);
            clean_from = i + 1;
        }
        i += 1;
    }
    buf.extend_from_slice(&bytes[clean_from..]);
}
