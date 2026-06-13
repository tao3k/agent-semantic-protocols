//! Byte-oriented text helpers shared by provider transport consumers.

use std::borrow::Cow;

use bstr::{BStr, ByteSlice};
use memchr::{memchr, memchr_iter, memchr2_iter};

/// Return a byte string view without assuming UTF-8 validity.
pub fn as_bstr(bytes: &[u8]) -> &BStr {
    bytes.as_bstr()
}

/// Render bytes as text with UTF-8 replacement for invalid sequences.
pub fn lossy(bytes: &[u8]) -> Cow<'_, str> {
    as_bstr(bytes).to_str_lossy()
}

/// Render bytes as an owned lossy UTF-8 string.
pub fn lossy_string(bytes: &[u8]) -> String {
    lossy(bytes).into_owned()
}

/// Render bytes as a lowercase lossy UTF-8 string.
pub fn lowercase_lossy_string(bytes: &[u8]) -> String {
    lossy(bytes).to_lowercase()
}

/// Find a single byte in a byte slice.
pub fn find_byte(needle: u8, haystack: &[u8]) -> Option<usize> {
    memchr(needle, haystack)
}

/// Trim ASCII whitespace from both ends of a byte slice.
pub fn trim_ascii(bytes: &[u8]) -> &[u8] {
    let mut start = 0;
    let mut end = bytes.len();
    while start < end && bytes[start].is_ascii_whitespace() {
        start += 1;
    }
    while end > start && bytes[end - 1].is_ascii_whitespace() {
        end -= 1;
    }
    &bytes[start..end]
}

/// Trim a trailing carriage return from an LF-delimited line.
pub fn trim_line_ending(bytes: &[u8]) -> &[u8] {
    bytes.strip_suffix(b"\r").unwrap_or(bytes)
}

/// Split stdin-style records on LF or NUL and trim ASCII whitespace.
pub fn split_lf_or_nul_records(bytes: &[u8]) -> impl Iterator<Item = &[u8]> {
    let mut start = 0;
    memchr2_iter(b'\n', b'\0', bytes)
        .chain(std::iter::once(bytes.len()))
        .map(move |end| {
            let record = trim_ascii(&bytes[start..end]);
            start = end.saturating_add(1);
            record
        })
}

/// Split LF-delimited text into borrowed lines, trimming a trailing CR.
pub fn split_lf_lines(bytes: &[u8]) -> impl Iterator<Item = &[u8]> {
    bytes.split(|byte| *byte == b'\n').map(trim_line_ending)
}

/// Return non-empty LF-delimited line slices, trimming a trailing CR.
pub fn line_slices(bytes: &[u8]) -> Vec<&[u8]> {
    let mut lines = Vec::new();
    let mut start = 0;
    for end in memchr_iter(b'\n', bytes) {
        lines.push(trim_line_ending(&bytes[start..end]));
        start = end + 1;
    }
    if start < bytes.len() {
        lines.push(trim_line_ending(&bytes[start..]));
    }
    lines
}
