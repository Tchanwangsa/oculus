//! When each phrase is on screen.

use super::phrase::{LONG_GAP, Phrase};
use crate::project::CuedWord;

/// A phrase appears this long before its first word.
const LEAD: f64 = 0.15;
/// A phrase followed by a long silence stays this long after its last word.
const HOLD: f64 = 0.4;

/// Sets each phrase's `show` and `hide`. A phrase shows `LEAD` before its
/// first word (never before the phrase before it hides) and stays until the
/// next one shows; across a silence of `LONG_GAP` or more, and after the
/// last phrase, it hides `HOLD` after its last word, so a long pause has no
/// caption.
pub(super) fn place(phrases: &mut [Phrase], words: &[CuedWord]) {
    let mut hidden = 0.0f64;
    for i in 0..phrases.len() {
        let first = &words[phrases[i].words.start];
        let last = &words[phrases[i].words.end - 1];
        let show = (first.start - LEAD).max(hidden).max(0.0);
        let hide = match phrases.get(i + 1) {
            Some(next) if words[next.words.start].start - last.end < LONG_GAP => {
                (words[next.words.start].start - LEAD).max(last.end)
            }
            _ => last.end + HOLD,
        };
        phrases[i].show = show;
        phrases[i].hide = hide.max(show);
        hidden = phrases[i].hide;
    }
}
