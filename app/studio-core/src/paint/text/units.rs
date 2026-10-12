//! Which reveal unit each byte of a text belongs to: its word, its
//! non-space character, or (by line) the line it was laid out on.

use crate::scene::RevealBy;

pub struct Units {
    by: RevealBy,
    /// Unit of the char starting at each byte (word and char modes);
    /// `None` for whitespace.
    of_byte: Vec<Option<u32>>,
    pub count: usize,
}

impl Units {
    pub fn new(text: &str, by: RevealBy, lines: usize) -> Units {
        let mut of_byte = Vec::new();
        let mut count = 0;
        if by == RevealBy::Line {
            count = lines;
        } else {
            of_byte = vec![None; text.len() + 1];
            let mut in_word = false;
            for (i, ch) in text.char_indices() {
                if ch.is_whitespace() {
                    in_word = false;
                    continue;
                }
                if by == RevealBy::Char || !in_word {
                    count += 1;
                }
                in_word = true;
                of_byte[i] = Some(count as u32 - 1);
            }
        }
        Units { by, of_byte, count }
    }

    /// The unit of the cluster starting at `byte` on line `line`.
    pub fn of(&self, byte: usize, line: usize) -> Option<usize> {
        match self.by {
            RevealBy::Line => Some(line),
            _ => self
                .of_byte
                .get(byte)
                .copied()
                .flatten()
                .map(|u| u as usize),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn words_and_chars_skip_whitespace() {
        let w = Units::new("ab  cd e", RevealBy::Word, 1);
        assert_eq!(w.count, 3);
        assert_eq!(
            (w.of(0, 0), w.of(1, 0), w.of(2, 0)),
            (Some(0), Some(0), None)
        );
        assert_eq!((w.of(4, 0), w.of(7, 0)), (Some(1), Some(2)));
        let c = Units::new("ab c", RevealBy::Char, 1);
        assert_eq!(c.count, 3);
        assert_eq!((c.of(1, 0), c.of(3, 0)), (Some(1), Some(2)));
        let l = Units::new("ab c", RevealBy::Line, 2);
        assert_eq!((l.count, l.of(3, 1)), (2, Some(1)));
    }
}
