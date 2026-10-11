//! Script words as the aligner spells them.

/// wav2vec2-base-960h's vocabulary: `<pad>` = 0 is the CTC blank, 1-3 are
/// unused specials, `|` = 4 separates words.
pub(super) const VOCAB: usize = 32;
pub(super) const BLANK: usize = 0;
pub(super) const SEPARATOR: usize = 4;

/// Each label's character, for reading back what the model heard.
pub(super) const LETTERS: &[u8; VOCAB] = b"????|ETAONIHSRDLUMWCFGYPBVK'XJQZ";

/// The label of a letter or `'`; None for anything the model cannot spell.
pub(super) fn label(ch: char) -> Option<usize> {
    if ch == '|' {
        return None;
    }
    LETTERS.iter().position(|&l| l != b'?' && l as char == ch)
}

/// The words the aligner is forced through, and for each the index of the
/// script word it came from.
#[derive(Debug, PartialEq)]
pub(super) struct Targets {
    pub words: Vec<String>,
    pub index: Vec<usize>,
}

/// Upper-case each script word and keep A-Z and `'`. A word left empty (a
/// dash, `&`) has nothing the model could match, so it is not a target.
pub(super) fn targets(script: &[String]) -> Targets {
    let mut out = Targets {
        words: Vec::new(),
        index: Vec::new(),
    };
    for (i, word) in script.iter().enumerate() {
        let spelled: String = word
            .to_uppercase()
            .chars()
            .filter(|&c| label(c).is_some())
            .collect();
        if !spelled.is_empty() {
            out.words.push(spelled);
            out.index.push(i);
        }
    }
    out
}
