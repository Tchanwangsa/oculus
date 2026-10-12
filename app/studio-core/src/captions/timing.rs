//! When each phrase is on screen, and which word is being said at a moment.

use super::phrase::{LONG_GAP, Phrase};
use crate::project::CuedWord;

/// A phrase appears this long before its first word.
const LEAD: f64 = 0.15;
/// A phrase followed by a long silence stays this long after its last word.
const HOLD: f64 = 0.4;
/// A word stays highlighted this long into the silence after it, unless
/// the next word starts first.
const LINGER: f64 = 0.3;

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

/// What the captions show at one moment: phrase `phrase`, with `current`
/// the word being said (`None` before its first word and in a silence), and
/// every word before `said` already said.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Moment {
    pub phrase: usize,
    pub current: Option<usize>,
    pub said: usize,
}

/// The moment at scene time `t`, or `None` when no phrase is on screen.
pub fn moment(phrases: &[Phrase], words: &[CuedWord], t: f64) -> Option<Moment> {
    let phrase = phrases.iter().position(|p| p.show <= t && t < p.hide)?;
    let range = phrases[phrase].words.clone();
    // The last word of the phrase that has started by `t`.
    let started = range.clone().rev().find(|&i| words[i].start <= t);
    let current = started.filter(|&i| t < words[i].end + LINGER);
    let said = started.map_or(range.start, |i| i + 1);
    Some(Moment {
        phrase,
        current,
        said,
    })
}
