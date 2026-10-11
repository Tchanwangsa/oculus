//! Character tests, as the parser's JavaScript makes them. The parser walks
//! UTF-8 bytes; every test that Lezer makes on one UTF-16 unit (`charCodeAt`,
//! a one-unit `slice`) is answered here for the char at a byte offset.

use super::tables::{JS_SPACE, PUNCTUATION};

/// Lezer's `space`: space, tab, LF, CR (as a char code, -1 for none).
pub(crate) fn space(c: i32) -> bool {
    c == 32 || c == 9 || c == 10 || c == 13
}

fn in_ranges(c: u32, ranges: &[(u16, u16)]) -> bool {
    if c > 0xffff {
        return false;
    }
    let c = c as u16;
    ranges
        .binary_search_by(|&(lo, hi)| {
            if hi < c {
                std::cmp::Ordering::Less
            } else if lo > c {
                std::cmp::Ordering::Greater
            } else {
                std::cmp::Ordering::Equal
            }
        })
        .is_ok()
}

/// JS `/\s/` (also what `trim` strips). An astral char is false: JS sees its
/// surrogate halves, which are not spaces.
pub(crate) fn js_space(c: char) -> bool {
    in_ranges(c as u32, JS_SPACE)
}

/// Lezer's `Punctuation` (`/[\p{S}|\p{P}]/u`) on the first UTF-16 unit of
/// `c`; astral chars are false, being a lone surrogate there.
pub(crate) fn js_punctuation(c: char) -> bool {
    in_ranges(c as u32, PUNCTUATION)
}

/// JS `\w` without the `u` flag.
pub(crate) fn word(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_'
}

/// UTF-16 units contributed by a UTF-8 byte: 1 for ASCII and the lead of a
/// 2- or 3-byte char, 2 for the lead of a 4-byte char, 0 for continuations.
pub(crate) fn units(b: u8) -> usize {
    match b {
        0..=0x7f => 1,
        0x80..=0xbf => 0,
        0xc0..=0xef => 1,
        _ => 2,
    }
}

/// `s.trimEnd()` in JS.
pub(crate) fn trim_end(s: &str) -> &str {
    s.trim_end_matches(js_space)
}

/// `s.trim() === ""` in JS.
pub(crate) fn is_blank(s: &str) -> bool {
    s.chars().all(js_space)
}

/// The char starting at byte `i` of `s`, if `i` is in range.
pub(crate) fn char_at(s: &str, i: usize) -> Option<char> {
    s.get(i..).and_then(|rest| rest.chars().next())
}

/// The char ending at byte `i` of `s`.
pub(crate) fn char_before(s: &str, i: usize) -> Option<char> {
    s.get(..i).and_then(|head| head.chars().next_back())
}

/// Byte offset `s[from..]` reaches after skipping JS whitespace.
pub(crate) fn skip_js_space(s: &str, from: usize) -> usize {
    let mut i = from;
    while let Some(c) = char_at(s, i) {
        if !js_space(c) {
            break;
        }
        i += c.len_utf8();
    }
    i
}

/// Maps byte offsets of one string to UTF-16 offsets.
pub(crate) struct Utf16Map<'a> {
    bytes: &'a [u8],
    /// UTF-16 units before each 64-byte block; empty for ASCII text.
    blocks: Vec<u32>,
}

impl<'a> Utf16Map<'a> {
    pub fn new(s: &'a str) -> Self {
        let bytes = s.as_bytes();
        let mut blocks = Vec::new();
        if !s.is_ascii() {
            let mut units_so_far = 0u32;
            for chunk in bytes.chunks(64) {
                blocks.push(units_so_far);
                units_so_far += chunk.iter().map(|&b| units(b) as u32).sum::<u32>();
            }
        }
        Utf16Map { bytes, blocks }
    }

    /// The UTF-16 offset of `byte`. A byte inside an astral char stands for
    /// the position between its surrogates (see `Line::find_column`).
    pub fn get(&self, byte: usize) -> usize {
        if self.blocks.is_empty() {
            return byte;
        }
        let mut start = byte;
        while start < self.bytes.len() && (0x80..0xc0).contains(&self.bytes[start]) {
            start -= 1;
        }
        let inside = usize::from(start != byte && self.bytes[start] >= 0xf0);
        let block = (start / 64).min(self.blocks.len() - 1);
        self.blocks[block] as usize
            + self.bytes[block * 64..start]
                .iter()
                .map(|&b| units(b))
                .sum::<usize>()
            + inside
    }
}
