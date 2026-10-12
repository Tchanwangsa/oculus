//! English (US) text to Kokoro phonemes, without spaCy or espeak: a port of
//! misaki 0.9.4's dictionary path (`misaki/en.py`) as the oracle in
//! `data/evals/studio-g2p/tools/a2c.py` runs it. Each word is read by the
//! user's lexicon, else misaki's gold and silver dictionaries with its
//! special cases, case handling and stem rules (plus `-ied` and British
//! `-ise`), else the guesser, one sub-token at a time. Nothing is dropped:
//! a word with a letter in it and no phonemes is an error naming it.
//!
//! The golden files under `tests/golden/` are the oracle's output; the
//! port matches them byte for byte.

mod chars;
mod dict;
pub mod guess;
mod resolve;
mod special;
mod split;
mod stem;
mod stress;
mod tag;
#[cfg(test)]
mod tests;
mod token;
mod word;

pub use dict::{Dictionary, Entry};
pub use resolve::phonemize;

/// Phonemes for a word or sub-token nothing else reads.
pub trait Guess {
    fn guess(&mut self, part: &str) -> Result<String, String>;
}

/// The user's respelling lexicon, read before the dictionary. Holds no
/// entries yet.
#[derive(Debug, Clone, Default)]
pub struct Lexicon {}

impl Lexicon {
    /// An entry's phonemes for `word`, or `None` to read it on.
    fn phonemes(&self, _dict: &Dictionary, _word: &str) -> Result<Option<String>, String> {
        Ok(None)
    }
}

/// Who read a word.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Source {
    Dict,
    /// The heteronym table picked the reading.
    Rule,
    Lexicon,
    Guess,
}

impl Source {
    pub fn as_str(self) -> &'static str {
        match self {
            Source::Dict => "dict",
            Source::Rule => "rule",
            Source::Lexicon => "lexicon",
            Source::Guess => "guess",
        }
    }
}

/// One whitespace word (punctuation split off) and how it reads.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WordRecord {
    pub word: String,
    pub phonemes: String,
    pub source: Source,
    /// The parts the guesser answered, right to left; empty unless `source`
    /// is `Guess`.
    pub guessed: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Phonemized {
    /// What Kokoro is given: the words' phonemes with the text's
    /// punctuation and whitespace (leading whitespace dropped).
    pub phonemes: String,
    pub words: Vec<WordRecord>,
}
