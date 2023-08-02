// Taken from orjson
use super::macros::{ffi, use_immortal};
use super::types::EMPTY_UNICODE;
use pyo3_ffi::*;

enum PyUnicodeKind {
    Ascii,
    OneByte,
    TwoByte,
    FourByte,
}

fn find_str_kind(buf: &str, num_chars: usize) -> PyUnicodeKind {
    if buf.len() == num_chars {
        PyUnicodeKind::Ascii
    } else if is_four_byte(buf) {
        PyUnicodeKind::FourByte
    } else if encoding_rs::mem::is_str_latin1(buf) {
        PyUnicodeKind::OneByte
    } else {
        PyUnicodeKind::TwoByte
    }
}

pub fn unicode_from_str(buf: &str) -> *mut pyo3_ffi::PyObject {
    if buf.is_empty() {
        use_immortal!(EMPTY_UNICODE)
    } else {
        let num_chars = bytecount::num_chars(buf.as_bytes());
        match find_str_kind(buf, num_chars) {
            PyUnicodeKind::Ascii => pyunicode_ascii(buf),
            PyUnicodeKind::OneByte => pyunicode_onebyte(buf, num_chars),
            PyUnicodeKind::TwoByte => pyunicode_twobyte(buf, num_chars),
            PyUnicodeKind::FourByte => pyunicode_fourbyte(buf, num_chars),
        }
    }
}

pub fn pyunicode_ascii(buf: &str) -> *mut pyo3_ffi::PyObject {
    let ptr = ffi!(PyUnicode_New(buf.len() as isize, 127));
    unsafe {
        let data_ptr = ptr.cast::<PyASCIIObject>().offset(1) as *mut u8;
        core::ptr::copy_nonoverlapping(buf.as_ptr(), data_ptr, buf.len());
        core::ptr::write(data_ptr.add(buf.len()), 0);
        ptr
    }
}

#[cold]
#[inline(never)]
pub fn pyunicode_onebyte(buf: &str, num_chars: usize) -> *mut pyo3_ffi::PyObject {
    let ptr = ffi!(PyUnicode_New(num_chars as isize, 255));
    unsafe {
        let mut data_ptr = ptr.cast::<PyCompactUnicodeObject>().offset(1) as *mut u8;
        for each in buf.chars().fuse() {
            std::ptr::write(data_ptr, each as u8);
            data_ptr = data_ptr.offset(1);
        }
        core::ptr::write(data_ptr, 0);
        ptr
    }
}

pub fn pyunicode_twobyte(buf: &str, num_chars: usize) -> *mut pyo3_ffi::PyObject {
    let ptr = ffi!(PyUnicode_New(num_chars as isize, 65535));
    unsafe {
        let mut data_ptr = ptr.cast::<PyCompactUnicodeObject>().offset(1) as *mut u16;
        for each in buf.chars().fuse() {
            std::ptr::write(data_ptr, each as u16);
            data_ptr = data_ptr.offset(1);
        }
        core::ptr::write(data_ptr, 0);
        ptr
    }
}

pub fn pyunicode_fourbyte(buf: &str, num_chars: usize) -> *mut pyo3_ffi::PyObject {
    let ptr = ffi!(PyUnicode_New(num_chars as isize, 1114111));
    unsafe {
        let mut data_ptr = ptr.cast::<PyCompactUnicodeObject>().offset(1) as *mut u32;
        for each in buf.chars().fuse() {
            std::ptr::write(data_ptr, each as u32);
            data_ptr = data_ptr.offset(1);
        }
        core::ptr::write(data_ptr, 0);
        ptr
    }
}

const STRIDE_SIZE: usize = 8;

pub fn is_four_byte(buf: &str) -> bool {
    let as_bytes = buf.as_bytes();
    let len = as_bytes.len();
    unsafe {
        let mut idx = 0;
        while idx < len.saturating_sub(STRIDE_SIZE) {
            let mut val: bool = false;
            val |= *as_bytes.get_unchecked(idx) > 239;
            val |= *as_bytes.get_unchecked(idx + 1) > 239;
            val |= *as_bytes.get_unchecked(idx + 2) > 239;
            val |= *as_bytes.get_unchecked(idx + 3) > 239;
            val |= *as_bytes.get_unchecked(idx + 4) > 239;
            val |= *as_bytes.get_unchecked(idx + 5) > 239;
            val |= *as_bytes.get_unchecked(idx + 6) > 239;
            val |= *as_bytes.get_unchecked(idx + 7) > 239;
            idx += STRIDE_SIZE;
            if val {
                return true;
            }
        }
        let mut ret = false;
        while idx < len {
            ret |= *as_bytes.get_unchecked(idx) > 239;
            idx += 1;
        }
        ret
    }
}
