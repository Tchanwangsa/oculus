//! Where a word is said in a video's narration, and how Kokoro reads it
//! there: what a suggestion records beside its reading.

use studio_core::Project;

use crate::studio::voice::g2p::{Phonemized, Source};
use crate::studio::voice::lint::{section_words, sections};

/// A word's first guessed place in the narration, else its first place.
#[derive(Debug, Clone, PartialEq)]
pub struct Located {
    /// The word as the script spells it (a possessive's stem).
    pub word: String,
    /// Its phonemes as Kokoro reads it now.
    pub original: String,
    /// Whether the guesser read it.
    pub guessed: bool,
    /// The spoken sentence it is in, whitespace collapsed.
    pub sentence: String,
    /// The scene's id and the script.md line (1-based).
    pub scene: String,
    pub line: usize,
}

/// Finds `word` (any case; `Word's` counts as `Word`) in `project`'s
/// narrated sections, reading each with `read`. A possessive's original
/// is the stem read alone. Fails when the narration never says it.
pub fn locate(
    project: &Project,
    mut read: impl FnMut(&str) -> Result<Phonemized, String>,
    word: &str,
) -> Result<Located, String> {
    let wanted = fold(word);
    let mut first: Option<Located> = None;
    for (_, section) in sections(project) {
        let phonemized = read(&section.text)?;
        let mut seen = 0;
        for (line, record) in section_words(section, &phonemized) {
            let folded = fold(&record.word);
            let stem = folded.strip_suffix("'s") == Some(wanted.as_str());
            if folded != wanted && !stem {
                continue;
            }
            seen += 1;
            let spelt = if stem {
                let mut chars: Vec<char> = record.word.chars().collect();
                chars.truncate(chars.len().saturating_sub(2));
                chars.into_iter().collect()
            } else {
                record.word.clone()
            };
            let original = if stem {
                read(&spelt)?.phonemes.trim().to_owned()
            } else {
                record.phonemes.clone()
            };
            let found = Located {
                sentence: sentence(&section.text, &spelt, seen),
                word: spelt,
                original,
                guessed: record.source == Source::Guess,
                scene: section.id.clone(),
                line,
            };
            if found.guessed {
                return Ok(found);
            }
            first.get_or_insert(found);
        }
    }
    first.ok_or_else(|| {
        format!(
            "\"{word}\" is not in the narration; suggest a reading only for a word the script says"
        )
    })
}

/// `’` as `'`, lower case.
fn fold(word: &str) -> String {
    word.replace('’', "'").to_lowercase()
}

/// The sentence holding the `nth` (1-based) whole-word `word`, as the
/// script spells it, in `text`, in any case; else the first sentence
/// holding it; else all of `text`.
fn sentence(text: &str, word: &str, nth: usize) -> String {
    let lowered = text.to_lowercase();
    // Lower-casing keeps byte offsets only when it keeps the length.
    let (haystack, needle) = if lowered.len() == text.len() {
        (lowered.as_str(), word.to_lowercase())
    } else {
        (text, word.to_owned())
    };
    let hits: Vec<usize> = haystack
        .match_indices(&needle)
        .map(|(at, _)| at)
        .filter(|&at| whole(haystack, at, needle.len()))
        .collect();
    let Some(&at) = hits.get(nth - 1).or(hits.first()) else {
        return collapse(text);
    };
    let starts = ends(text);
    let start = starts
        .iter()
        .rev()
        .find(|&&e| e <= at)
        .copied()
        .unwrap_or(0);
    let end = starts
        .iter()
        .find(|&&e| e > at)
        .copied()
        .unwrap_or(text.len());
    collapse(&text[start..end])
}

/// No letter or digit right before `at` or right after the match.
fn whole(text: &str, at: usize, len: usize) -> bool {
    let before = text[..at].chars().next_back();
    let after = text[at + len..].chars().next();
    !before.is_some_and(char::is_alphanumeric) && !after.is_some_and(char::is_alphanumeric)
}

/// The byte offset just past each sentence end (`.`, `!`, `?`, `…` and any
/// closing quote or bracket) that whitespace follows.
fn ends(text: &str) -> Vec<usize> {
    let chars: Vec<(usize, char)> = text.char_indices().collect();
    let mut out = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        if matches!(chars[i].1, '.' | '!' | '?' | '…') {
            let mut j = i + 1;
            while j < chars.len() && matches!(chars[j].1, '"' | '\'' | '”' | '’' | ')' | ']') {
                j += 1;
            }
            if j < chars.len() && chars[j].1.is_whitespace() {
                out.push(chars[j].0);
            }
            i = j;
        } else {
            i += 1;
        }
    }
    out
}

/// Whitespace runs as one space, trimmed.
fn collapse(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}
