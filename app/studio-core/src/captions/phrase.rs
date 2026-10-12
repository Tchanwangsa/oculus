//! A scene's words cut into phrases, one caption each, and each phrase into
//! balanced lines. Phrases break first where the narration does (a
//! `[pause]`, a silence, a sentence's end), then at clause punctuation,
//! else wherever the next word would overflow; a phrase never ends with a
//! lone word left over when the one before can lend it a partner.

use std::ops::Range;

use crate::project::{CuedPause, CuedWord};

/// Silence between two words that always ends a phrase, in seconds.
pub(super) const LONG_GAP: f64 = 0.6;
/// Silence that is a clause break, like a comma.
const SHORT_GAP: f64 = 0.25;

/// How strongly the narration breaks between two words.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Break {
    None,
    /// A comma, semicolon, colon or dash, or a short silence.
    Clause,
    /// A sentence's end, a `[pause]` or a long silence.
    Hard,
}

/// One caption: words `words`, set as `lines` (word ranges), on screen from
/// `show` to `hide` (scene seconds).
#[derive(Clone, Debug, PartialEq)]
pub struct Phrase {
    pub words: Range<usize>,
    pub lines: Vec<Range<usize>>,
    pub show: f64,
    pub hide: f64,
}

/// The break after word `i` (before word `i + 1`).
fn break_after(words: &[CuedWord], pauses: &[CuedPause], i: usize) -> Break {
    let text = words[i]
        .text
        .trim_end_matches(['"', '\'', ')', ']', '”', '’']);
    let gap = words[i + 1].start - words[i].end;
    if pauses.iter().any(|p| p.after == Some(i))
        || gap >= LONG_GAP
        || text.ends_with(['.', '!', '?', '…'])
    {
        Break::Hard
    } else if gap >= SHORT_GAP
        || text.ends_with([',', ';', ':', '—', '–'])
        || matches!(words[i + 1].text.as_str(), "—" | "–" | "-")
    {
        Break::Clause
    } else {
        Break::None
    }
}

/// Characters of words `range` set on one line, a space between each.
pub(super) fn chars(words: &[CuedWord], range: Range<usize>) -> usize {
    let n = range.len();
    words[range]
        .iter()
        .map(|w| w.text.chars().count())
        .sum::<usize>()
        + n.saturating_sub(1)
}

/// `words` as phrases of at most `max_lines` lines of `line_chars`
/// characters (a single longer word gets a phrase to itself), with their
/// times.
pub fn phrases(
    words: &[CuedWord],
    pauses: &[CuedPause],
    line_chars: usize,
    max_lines: usize,
) -> Vec<Phrase> {
    let capacity = line_chars * max_lines + max_lines.saturating_sub(1);
    let mut cuts = Vec::new();
    let mut start = 0;
    for i in 0..words.len() {
        if i + 1 == words.len() || break_after(words, pauses, i) == Break::Hard {
            let fit = Fit {
                line_chars,
                max_lines,
                capacity,
            };
            split(words, pauses, start..i + 1, fit, &mut cuts);
            start = i + 1;
        }
    }
    let mut out: Vec<Phrase> = cuts
        .into_iter()
        .map(|r| Phrase {
            lines: lines(words, pauses, r.clone(), line_chars, max_lines),
            words: r,
            show: 0.0,
            hide: 0.0,
        })
        .collect();
    super::timing::place(&mut out, words);
    out
}

/// What a phrase may hold.
#[derive(Clone, Copy)]
struct Fit {
    line_chars: usize,
    max_lines: usize,
    /// Characters in `max_lines` full lines.
    capacity: usize,
}

/// Lines words `range` take, each filled as far as it goes: the fewest
/// lines of `line_chars` that hold them.
fn line_count(words: &[CuedWord], range: Range<usize>, line_chars: usize) -> usize {
    let mut count = 0;
    let mut start = range.start;
    while start < range.end {
        let mut end = start + 1;
        while end < range.end && chars(words, start..end + 1) <= line_chars {
            end += 1;
        }
        count += 1;
        start = end;
    }
    count
}

/// Cuts `chunk` (no hard break inside) into phrases that fit `fit`'s lines:
/// each takes as many words as fit, ending at the last clause break that
/// leaves it at least half full, and gives a word back when the rest would
/// be one word alone.
fn split(
    words: &[CuedWord],
    pauses: &[CuedPause],
    chunk: Range<usize>,
    fit: Fit,
    out: &mut Vec<Range<usize>>,
) {
    let capacity = fit.capacity;
    let mut start = chunk.start;
    while start < chunk.end {
        let mut end = start + 1;
        while end < chunk.end && line_count(words, start..end + 1, fit.line_chars) <= fit.max_lines
        {
            end += 1;
        }
        if end < chunk.end {
            let clause = (start + 2..=end).rev().find(|&k| {
                break_after(words, pauses, k - 1) >= Break::Clause
                    && chars(words, start..k) * 2 >= capacity
            });
            end = clause.unwrap_or(end);
            if chunk.end - end == 1 && end - start >= 3 {
                end -= 1;
            }
        }
        out.push(start..end);
        start = end;
    }
}

/// How much longer, in characters, the longest line may get when a cut
/// moves to a clause break: "eleven, / wrap round" reads better than
/// "eleven, wrap / round" even when less even.
const CLAUSE_SLACK: usize = 9;

/// Words `phrase` as at most `max_lines` lines: as few as hold it at
/// `line_chars` a line, the cut points chosen so the longest line is as
/// short as it can be (a tie puts more on the later line), then each cut
/// moved to the nearest clause break that keeps every line within
/// `line_chars` and the longest within `CLAUSE_SLACK` of that.
fn lines(
    words: &[CuedWord],
    pauses: &[CuedPause],
    phrase: Range<usize>,
    line_chars: usize,
    max_lines: usize,
) -> Vec<Range<usize>> {
    let n = phrase.len();
    let wanted = line_count(words, phrase.clone(), line_chars).clamp(1, max_lines.min(n));
    if wanted == 1 {
        return vec![phrase];
    }
    // best[l][j]: the least longest line setting the first j words in l
    // lines, and where its last line starts.
    let at = |i: usize| phrase.start + i;
    let mut best = vec![vec![(usize::MAX, 0usize); n + 1]; wanted + 1];
    best[0][0] = (0, 0);
    for l in 1..=wanted {
        for j in l..=n {
            for i in (l - 1)..j {
                let (prev, _) = best[l - 1][i];
                if prev == usize::MAX {
                    continue;
                }
                let worst = prev.max(chars(words, at(i)..at(j)));
                if worst < best[l][j].0 {
                    best[l][j] = (worst, i);
                }
            }
        }
    }
    let mut cuts = vec![n];
    let mut j = n;
    for l in (1..=wanted).rev() {
        j = best[l][j].1;
        cuts.push(j);
    }
    cuts.reverse();
    let longest = |cuts: &[usize]| {
        cuts.windows(2)
            .map(|w| chars(words, at(w[0])..at(w[1])))
            .max()
            .unwrap_or(0)
    };
    let limit = (longest(&cuts) + CLAUSE_SLACK).min(line_chars.max(longest(&cuts)));
    for k in 1..cuts.len() - 1 {
        let clause = |c: usize| break_after(words, pauses, at(c) - 1) >= Break::Clause;
        if clause(cuts[k]) {
            continue;
        }
        let mut options: Vec<usize> = (cuts[k - 1] + 1..cuts[k + 1])
            .filter(|&c| clause(c))
            .collect();
        options.sort_by_key(|&c| c.abs_diff(cuts[k]));
        if let Some(c) = options.into_iter().find(|&c| {
            let mut moved = cuts.clone();
            moved[k] = c;
            longest(&moved) <= limit
        }) {
            cuts[k] = c;
        }
    }
    cuts.windows(2).map(|w| at(w[0])..at(w[1])).collect()
}
