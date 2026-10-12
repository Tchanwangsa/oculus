//! A scene's phonemes as Kokoro inputs: token ids in pieces short enough
//! for the model, cut at sentence ends, then at commas, semicolons and
//! colons inside a sentence that is too long alone. A clause still too
//! long is an error naming its sentence, as is a character outside
//! Kokoro's vocabulary (Kokoro would drop it silently) and the pad `$`.

use super::model::STYLE_ROWS;
use super::vocab::{self, PAD};
use crate::studio::voice::g2p::Phonemized;

/// Most tokens one input holds, pads aside: the voice file has a style row
/// per input length, the last for 509.
pub const MAX_TOKENS: usize = STYLE_ROWS - 1;

const SENTENCE_ENDS: &str = ".!?…";
const CLAUSE_ENDS: &str = ",;:";
/// May follow a sentence's end before the space: `.”`, `?)`.
const CLOSERS: &str = "\"”)'’";

/// One input: its token ids without the pads, and the words it says, for
/// messages.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Chunk {
    pub ids: Vec<i64>,
    pub words: String,
}

/// The scene's inputs, in order. Whitespace runs read as one space.
pub fn chunks(phonemized: &Phonemized) -> Result<Vec<Chunk>, String> {
    chunks_under(phonemized, MAX_TOKENS)
}

pub(super) fn chunks_under(phonemized: &Phonemized, max: usize) -> Result<Vec<Chunk>, String> {
    let ps = phonemized.phonemes.as_str();
    let spans = word_spans(phonemized);
    check_chars(ps, &spans)?;
    let mut pieces = Vec::new();
    for sentence in split(ps, 0..ps.len(), SENTENCE_ENDS) {
        if tokens(&ps[sentence.clone()]).len() <= max {
            pieces.push(sentence);
            continue;
        }
        for clause in split(ps, sentence.clone(), CLAUSE_ENDS) {
            let n = tokens(&ps[clause.clone()]).len();
            if n > max {
                return Err(format!(
                    "a sentence is too long for Kokoro: one part of it is {n} phonemes with no comma, semicolon or colon to cut at, and an input holds {max}; split it: \"{}\"",
                    words_in(&spans, sentence)
                ));
            }
            pieces.push(clause);
        }
    }

    let mut out = Vec::new();
    let mut current: Option<std::ops::Range<usize>> = None;
    for piece in pieces {
        current = match current {
            Some(c) if tokens(&ps[c.start..piece.end]).len() <= max => Some(c.start..piece.end),
            Some(c) => {
                out.push(c);
                Some(piece)
            }
            None => Some(piece),
        };
    }
    out.extend(current);
    let chunks: Vec<Chunk> = out
        .into_iter()
        .map(|range| Chunk {
            ids: tokens(&ps[range.clone()]),
            words: words_in(&spans, range),
        })
        .filter(|c| !c.ids.is_empty())
        .collect();
    if chunks.is_empty() {
        return Err("there is nothing to say: the text has no words".to_owned());
    }
    Ok(chunks)
}

/// `ps` trimmed, whitespace runs as one space, as ids. Every character was
/// checked by [`check_chars`].
fn tokens(ps: &str) -> Vec<i64> {
    let mut ids = Vec::new();
    let mut space = false;
    for c in ps.trim().chars() {
        if c.is_whitespace() {
            space = true;
            continue;
        }
        if space {
            ids.extend(vocab::id(' '));
            space = false;
        }
        ids.extend(vocab::id(c));
    }
    ids
}

/// Each word's byte range in the phoneme string, in order.
fn word_spans(phonemized: &Phonemized) -> Vec<(std::ops::Range<usize>, &str)> {
    let ps = phonemized.phonemes.as_str();
    let mut spans = Vec::new();
    let mut from = 0;
    for w in &phonemized.words {
        if let Some(at) = ps[from..].find(w.phonemes.as_str()) {
            let start = from + at;
            from = start + w.phonemes.len();
            spans.push((start..from, w.word.as_str()));
        }
    }
    spans
}

/// The words starting inside `range`, space-joined.
fn words_in(spans: &[(std::ops::Range<usize>, &str)], range: std::ops::Range<usize>) -> String {
    spans
        .iter()
        .filter(|(s, _)| range.contains(&s.start))
        .map(|(_, w)| *w)
        .collect::<Vec<_>>()
        .join(" ")
}

/// Every character is Kokoro's and none is the pad; else an error naming
/// the word it is in.
fn check_chars(ps: &str, spans: &[(std::ops::Range<usize>, &str)]) -> Result<(), String> {
    for (at, c) in ps.char_indices() {
        if c.is_whitespace() || (vocab::contains(c) && c != PAD) {
            continue;
        }
        let place = spans
            .iter()
            .find(|(s, _)| s.contains(&at))
            .map(|(_, w)| format!(" in \"{w}\""))
            .unwrap_or_default();
        return Err(if c == PAD {
            format!("`$`{place} is Kokoro's pad and cannot be said; spell it out")
        } else {
            format!("the phoneme {c:?}{place} is not in Kokoro's vocabulary")
        });
    }
    Ok(())
}

/// `range` of `ps` cut after each mark in `marks` (and any closers after
/// it) that whitespace or the end follows.
fn split(ps: &str, range: std::ops::Range<usize>, marks: &str) -> Vec<std::ops::Range<usize>> {
    let text = &ps[range.clone()];
    let mut cuts = Vec::new();
    let mut chars = text.char_indices().peekable();
    while let Some((_, c)) = chars.next() {
        if !marks.contains(c) {
            continue;
        }
        while let Some(&(_, next)) = chars.peek() {
            if !CLOSERS.contains(next) {
                break;
            }
            chars.next();
        }
        if let Some(&(at, next)) = chars.peek() {
            if next.is_whitespace() {
                cuts.push(at);
            }
        }
    }
    let mut out = Vec::new();
    let mut start = 0;
    for cut in cuts {
        out.push(range.start + start..range.start + cut);
        start = cut;
    }
    out.push(range.start + start..range.end);
    out
}
