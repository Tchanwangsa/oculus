//! Byte ↔ UTF-16 offsets. The parse tree's locations are bytes of the
//! formula; CodeMirror and the DOM count UTF-16 code units. Convert at the
//! wasm boundary (`oculus-math`'s `boundary`), nowhere else.

use core::iter::repeat_n;

/// The UTF-16 offset of byte `byte`; `None` off a char boundary or past
/// the end.
#[must_use]
pub fn utf16_offset(source: &str, byte: usize) -> Option<usize> {
    source
        .get(..byte)
        .map(|before| before.chars().map(char::len_utf16).sum())
}

/// The byte offset of UTF-16 offset `unit`; `None` inside a surrogate
/// pair or past the end.
#[must_use]
pub fn byte_offset(source: &str, unit: usize) -> Option<usize> {
    let mut units = 0;
    for (byte, c) in source.char_indices() {
        if units >= unit {
            return (units == unit).then_some(byte);
        }
        units += c.len_utf16();
    }
    (units == unit).then_some(source.len())
}

/// One source's UTF-16 offsets by byte, for converting many offsets
/// (every stop of a formula) in one pass over it.
#[derive(Clone, Debug)]
pub struct Units(Vec<u32>);

impl Units {
    #[must_use]
    pub fn new(source: &str) -> Self {
        let mut table = Vec::with_capacity(source.len() + 1);
        let mut units = 0;
        for c in source.chars() {
            table.extend(repeat_n(units, c.len_utf8()));
            units += c.len_utf16() as u32;
        }
        table.push(units);
        Self(table)
    }

    /// The UTF-16 offset of byte `byte`, which must be a char boundary of
    /// the source (a byte inside a character gives that character's
    /// start); past the end, the source's length.
    #[must_use]
    pub fn of(&self, byte: usize) -> u32 {
        self.0
            .get(byte)
            .or_else(|| self.0.last())
            .copied()
            .unwrap_or(0)
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::non_ascii_literal)]

    use super::{Units, byte_offset, utf16_offset};

    #[test]
    fn thai_in_text_round_trips() {
        let source = r"\text{ไทย}x";
        // Each Thai character is 3 bytes and 1 unit.
        assert_eq!(utf16_offset(source, 6), Some(6));
        assert_eq!(utf16_offset(source, 9), Some(7));
        assert_eq!(utf16_offset(source, 15), Some(9));
        assert_eq!(utf16_offset(source, 7), None);
        for (byte, _) in source.char_indices() {
            let unit = utf16_offset(source, byte).unwrap_or_default();
            assert_eq!(byte_offset(source, unit), Some(byte));
        }
        assert_eq!(byte_offset(source, 11), Some(source.len()));
        assert_eq!(byte_offset(source, 12), None);
    }

    #[test]
    fn astral_characters_are_two_units() {
        let source = "a𝔸b";
        assert_eq!(utf16_offset(source, 1), Some(1));
        assert_eq!(utf16_offset(source, 5), Some(3));
        assert_eq!(utf16_offset(source, 6), Some(4));
        assert_eq!(byte_offset(source, 3), Some(5));
        // Between the surrogates is no byte offset.
        assert_eq!(byte_offset(source, 2), None);
    }

    #[test]
    fn the_table_agrees_with_utf16_offset() {
        for source in [r"\text{ไทย}x", "a𝔸b", "", "x^2"] {
            let units = Units::new(source);
            for (byte, _) in source.char_indices().chain([(source.len(), ' ')]) {
                assert_eq!(Some(units.of(byte) as usize), utf16_offset(source, byte));
            }
        }
    }
}
