//! Speech the script does not account for. A word the TTS said but the
//! script lacks lowers no word's confidence: the forced path parks on
//! blanks across it. It shows instead as frames between words whose best
//! label is a letter.

use super::targets::{BLANK, LETTERS, SEPARATOR, VOCAB};
use super::viterbi::Span;

/// Unscripted speech before span `next` (`spans.len()` for after the last).
#[derive(Debug, PartialEq)]
pub(super) struct Heard {
    pub next: usize,
    /// The gap's frames, end exclusive.
    pub start_frame: usize,
    pub end_frame: usize,
    /// Frames whose best label is a letter or `'`.
    pub letter_frames: usize,
    /// A greedy decode of the gap: repeats collapsed, blanks dropped.
    pub heard: String,
}

/// Every gap between spans (and before the first, after the last) with at
/// least one letter frame.
pub(super) fn unscripted(em: &[f32], spans: &[Span]) -> Vec<Heard> {
    let t_len = em.len() / VOCAB;
    let best = |t: usize| {
        let row = &em[t * VOCAB..(t + 1) * VOCAB];
        (0..VOCAB).fold(0, |b, k| if row[k] > row[b] { k } else { b })
    };
    let mut out = Vec::new();
    for next in 0..=spans.len() {
        let start = if next == 0 {
            0
        } else {
            spans[next - 1].end_frame + 1
        };
        let end = if next == spans.len() {
            t_len
        } else {
            spans[next].start_frame
        };
        let (mut letters, mut heard, mut prev) = (0, String::new(), usize::MAX);
        for t in start..end {
            let id = best(t);
            if id != BLANK && id != SEPARATOR {
                letters += 1;
            }
            if id != prev && id != BLANK {
                heard.push(LETTERS[id] as char);
            }
            prev = id;
        }
        if letters > 0 {
            out.push(Heard {
                next,
                start_frame: start,
                end_frame: end,
                letter_frames: letters,
                heard: heard
                    .split('|')
                    .filter(|w| !w.is_empty())
                    .collect::<Vec<_>>()
                    .join(" "),
            });
        }
    }
    out
}
