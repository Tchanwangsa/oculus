//! The guesser's two alphabets, read from the pack's config, and how a word
//! becomes encoder ids.

use std::collections::HashMap;

use serde::Deserialize;
use unicode_normalization::char::canonical_combining_class;
use unicode_normalization::UnicodeNormalization;

/// The decoder's first input.
pub const START: i64 = 1;
/// The decoder's end of output; also closes the encoder's input.
pub const STOP: i64 = 2;
/// A grapheme outside the model's alphabet.
pub const UNKNOWN: i64 = 3;
/// Ids 0-3 are padding, start, stop and unknown; every id above is a character.
const RESERVED: usize = 4;

#[derive(Debug)]
pub struct Vocab {
    graphemes: HashMap<char, i64>,
    phonemes: Vec<char>,
}

#[derive(Deserialize)]
struct Config {
    grapheme_chars: String,
    phoneme_chars: String,
    vocab_size: usize,
}

impl Vocab {
    /// The alphabets from the model's `config.json`: `grapheme_chars` and
    /// `phoneme_chars`, where a character's index is its id.
    pub fn parse(json: &str) -> Result<Self, String> {
        let config: Config = serde_json::from_str(json).map_err(|e| e.to_string())?;
        let graphemes: Vec<char> = config.grapheme_chars.chars().collect();
        let phonemes: Vec<char> = config.phoneme_chars.chars().collect();
        for (name, chars) in [("grapheme_chars", &graphemes), ("phoneme_chars", &phonemes)] {
            if chars.len() <= RESERVED || chars.len() > config.vocab_size {
                return Err(format!(
                    "{name} has {} characters; the model's vocabulary is {}",
                    chars.len(),
                    config.vocab_size
                ));
            }
        }
        // The reserved slots hold placeholders, not graphemes; a later
        // duplicate wins, as in the Python the goldens came from.
        let graphemes = graphemes
            .into_iter()
            .enumerate()
            .skip(RESERVED)
            .map(|(id, c)| (c, id as i64))
            .collect();
        Ok(Self {
            graphemes,
            phonemes,
        })
    }

    /// `[START] + one id per character + [STOP]`; a character outside the
    /// alphabet is [`UNKNOWN`]. Takes a [`normalise`]d word.
    pub fn encode(&self, word: &str) -> Vec<i64> {
        let mut ids = vec![START];
        ids.extend(
            word.chars()
                .map(|c| self.graphemes.get(&c).copied().unwrap_or(UNKNOWN)),
        );
        ids.push(STOP);
        ids
    }

    /// The phoneme an output id stands for; `None` for a reserved id or one
    /// past the alphabet.
    pub fn phoneme(&self, id: i64) -> Option<char> {
        usize::try_from(id)
            .ok()
            .filter(|&i| i >= RESERVED)
            .and_then(|i| self.phonemes.get(i).copied())
    }
}

/// The model's alphabet is ASCII letters and `' - .`: a curly apostrophe
/// becomes straight, and NFKD plus dropping combining marks strips accents
/// (`Gödel` -> `Godel`). Case is kept.
pub fn normalise(word: &str) -> String {
    word.replace('\u{2019}', "'")
        .nfkd()
        .filter(|&c| canonical_combining_class(c) == 0)
        .collect()
}
