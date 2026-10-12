mod phrasing;
mod style;
mod subtitles;
mod timing;

use crate::project::{CuedPause, CuedWord};

/// Words spoken back to back, `dur` seconds each from `start`, with any
/// `(after word, seconds)` silences added.
fn spoken(text: &str, start: f64, dur: f64, silences: &[(usize, f64)]) -> Vec<CuedWord> {
    let mut t = start;
    text.split(' ')
        .enumerate()
        .map(|(i, w)| {
            let word = CuedWord {
                text: w.to_owned(),
                start: t,
                end: t + dur,
            };
            t += dur + silences.iter().find(|s| s.0 == i).map_or(0.0, |s| s.1);
            word
        })
        .collect()
}

/// Each phrase's words joined, its lines split by `/`.
fn shown(words: &[CuedWord], phrases: &[super::Phrase]) -> Vec<String> {
    phrases
        .iter()
        .map(|p| {
            let lines: Vec<String> = p
                .lines
                .iter()
                .map(|l| {
                    let w: Vec<&str> = words[l.clone()].iter().map(|w| w.text.as_str()).collect();
                    w.join(" ")
                })
                .collect();
            lines.join(" / ")
        })
        .collect()
}

fn pause_after(i: usize, words: &[CuedWord]) -> CuedPause {
    CuedPause {
        after: Some(i),
        start: words[i].end,
        end: words[i + 1].start,
    }
}
