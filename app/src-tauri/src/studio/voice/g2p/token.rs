//! A token as misaki's `MToken` carries it through the word path, and the
//! right-to-left context (`TokenContext`) a word's reading can depend on.

use super::stress::{is_consonant, is_vowel};

/// Punctuation that ends a clause: a word before it has no next sound.
pub const NON_QUOTE_PUNCTS: &str = ";:,.!?—…";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Token {
    pub text: String,
    /// A Penn tag (`DT`, `PRP`, `.`, `-LRB-`, ...) or empty.
    pub tag: String,
    /// The whitespace written after it.
    pub whitespace: String,
    /// `None` until something resolves it; `Some("")` reads as nothing.
    pub phonemes: Option<String>,
    /// A sub-token joined to the one before by a space.
    pub prespace: bool,
}

impl Token {
    pub fn new(text: &str, tag: &str, whitespace: &str) -> Self {
        Self {
            text: text.to_string(),
            tag: tag.to_string(),
            whitespace: whitespace.to_string(),
            phonemes: None,
            prespace: false,
        }
    }
}

/// What follows the word being read, built from right to left.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Ctx {
    /// Whether the next sound is a vowel; `None` at a clause end or the end.
    pub future_vowel: Option<bool>,
    /// The next word is `to`.
    pub future_to: bool,
}

/// misaki's `token_context`: the first vowel, consonant or clause mark of
/// the word just read decides `future_vowel`; a word with no phonemes or
/// none of those leaves it as it was.
pub fn token_context(ctx: Ctx, phonemes: Option<&str>, text: &str, tag: &str) -> Ctx {
    let mut future_vowel = ctx.future_vowel;
    if let Some(ps) = phonemes.filter(|p| !p.is_empty()) {
        if let Some(c) = ps
            .chars()
            .find(|&c| is_vowel(c) || is_consonant(c) || NON_QUOTE_PUNCTS.contains(c))
        {
            future_vowel = if NON_QUOTE_PUNCTS.contains(c) {
                None
            } else {
                Some(is_vowel(c))
            };
        }
    }
    let future_to = matches!(text, "to" | "To") || (text == "TO" && matches!(tag, "TO" | "IN"));
    Ctx {
        future_vowel,
        future_to,
    }
}
