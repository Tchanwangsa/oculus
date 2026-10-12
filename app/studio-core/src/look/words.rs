//! The narration's words over a stretch of a scene, quoted, so a warning
//! or a beat says what is being said while it happens.

use crate::project::CuedWord;

/// Most words a span quotes before it cuts with `…`.
const SPAN_WORDS: usize = 12;
/// Words a beat quotes.
const BEAT_WORDS: usize = 3;

/// The words starting in `[a, b]`, quoted, or `None` for none.
pub(crate) fn during(words: &[CuedWord], a: f64, b: f64) -> Option<String> {
    let said: Vec<&str> = words
        .iter()
        .filter(|w| w.start >= a - 1e-6 && w.start <= b + 1e-6)
        .map(|w| w.text.as_str())
        .collect();
    if said.is_empty() {
        return None;
    }
    let cut = said.len() > SPAN_WORDS;
    let shown = said[..said.len().min(SPAN_WORDS)].join(" ");
    Some(format!("\"{shown}{}\"", if cut { " …" } else { "" }))
}

/// ` while the narration says "…"`, or nothing when no word is said.
pub(crate) fn while_said(words: &[CuedWord], a: f64, b: f64) -> String {
    during(words, a, b)
        .map(|w| format!(" while the narration says {w}"))
        .unwrap_or_default()
}

/// A few words from the one being said at `t`, `…` marking a cut on
/// either side; empty before the first word starts or after the last ends.
pub(crate) fn at(words: &[CuedWord], t: f64) -> String {
    let Some(first) = words.first() else {
        return String::new();
    };
    let last = words.last().map_or(0.0, |w| w.end);
    if t < first.start - 0.5 || t > last {
        return String::new();
    }
    let i = words.iter().rposition(|w| w.start <= t + 1e-6).unwrap_or(0);
    let end = (i + BEAT_WORDS).min(words.len());
    let text: Vec<&str> = words[i..end].iter().map(|w| w.text.as_str()).collect();
    format!(
        "\"{}{}{}\"",
        if i > 0 { "… " } else { "" },
        text.join(" "),
        if end < words.len() { " …" } else { "" }
    )
}
