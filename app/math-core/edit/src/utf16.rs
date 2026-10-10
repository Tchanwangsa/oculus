//! Byte ↔ UTF-16 offsets. The parse tree's locations are bytes of the
//! formula; CodeMirror and the DOM count UTF-16 code units. Convert at the
//! wasm boundary, nowhere else.

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

#[cfg(test)]
mod tests {
    #![allow(clippy::non_ascii_literal)]

    use super::{byte_offset, utf16_offset};

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
}
