//! CTC forced alignment: the best path through the emissions that spells
//! the target words in order. A port of the spike's `align.rs` (itself of
//! `align_ctc.py`), with its states and tie-breaks, which matched Python
//! frame for frame.

use super::targets::{label, BLANK, SEPARATOR, VOCAB};
use crate::studio::NarrationError;

const NEG: f64 = -1e18;

/// Where the path sits on one word's letters, in 20 ms frames (inclusive).
#[derive(Debug, Clone, PartialEq)]
pub(super) struct Span {
    pub start_frame: usize,
    pub end_frame: usize,
    /// Mean log-prob of the frames the path spends on the word's letters.
    pub mean_logp: f64,
}

/// Viterbi over blank-interleaved states (2N+1 for N labels, words joined
/// by `|`). A repeated label needs a blank between; a skip over a blank is
/// allowed only between different labels. Ties go to stay, then advance,
/// then skip; the path ends on the last label or the trailing blank,
/// preferring the blank on a tie. `em` is `[t, VOCAB]` log-probs.
pub(super) fn force_align(em: &[f32], words: &[String]) -> Result<Vec<Span>, NarrationError> {
    let mut toks = Vec::new();
    let mut word_of: Vec<Option<usize>> = Vec::new();
    for (i, w) in words.iter().enumerate() {
        if i > 0 {
            toks.push(SEPARATOR);
            word_of.push(None);
        }
        for ch in w.chars() {
            toks.push(label(ch).expect("targets() keeps only labelled characters"));
            word_of.push(Some(i));
        }
    }
    let t_len = em.len() / VOCAB;
    let n = toks.len();
    if n == 0 {
        return Err(NarrationError::EmptyScript);
    }
    if t_len < n {
        return Err(NarrationError::Unalignable(format!(
            "{t_len} frames of audio cannot hold {n} letters"
        )));
    }
    let s_len = 2 * n + 1;
    let lab: Vec<usize> = (0..s_len)
        .map(|s| if s % 2 == 0 { BLANK } else { toks[s / 2] })
        .collect();
    let can_skip: Vec<bool> = (0..s_len)
        .map(|s| s >= 3 && s % 2 == 1 && lab[s] != lab[s - 2])
        .collect();

    let mut prev = vec![NEG; s_len];
    let mut cur = vec![NEG; s_len];
    let mut back = Steps::new(t_len, s_len);
    prev[0] = em[BLANK] as f64;
    prev[1] = em[lab[1]] as f64;
    for t in 1..t_len {
        let row = &em[t * VOCAB..(t + 1) * VOCAB];
        for s in 0..s_len {
            let mut best = prev[s];
            let mut k = 0u8;
            if s >= 1 && prev[s - 1] > best {
                best = prev[s - 1];
                k = 1;
            }
            if can_skip[s] && prev[s - 2] > best {
                best = prev[s - 2];
                k = 2;
            }
            cur[s] = best + row[lab[s]] as f64;
            back.set(t, s, k);
        }
        std::mem::swap(&mut prev, &mut cur);
    }
    let mut s = if prev[s_len - 1] > prev[s_len - 2] {
        s_len - 1
    } else {
        s_len - 2
    };
    if prev[s] <= NEG / 2.0 {
        return Err(NarrationError::Unalignable(
            "no path through the audio spells the script".into(),
        ));
    }
    let mut path = vec![0usize; t_len];
    for t in (0..t_len).rev() {
        path[t] = s;
        s -= back.get(t, s);
    }

    let mut first = vec![usize::MAX; words.len()];
    let mut last = vec![0usize; words.len()];
    let mut logp_sum = vec![0f64; words.len()];
    let mut logp_count = vec![0usize; words.len()];
    for (t, &s) in path.iter().enumerate() {
        if s % 2 == 0 {
            continue;
        }
        if let Some(w) = word_of[s / 2] {
            first[w] = first[w].min(t);
            last[w] = t;
            logp_sum[w] += em[t * VOCAB + lab[s]] as f64;
            logp_count[w] += 1;
        }
    }
    Ok((0..words.len())
        .map(|w| Span {
            start_frame: first[w],
            end_frame: last[w],
            mean_logp: logp_sum[w] / logp_count[w] as f64,
        })
        .collect())
}

/// Each frame and state's step into it (0 stay, 1 advance, 2 skip), four to
/// a byte: a byte each is about 6 MB per minute squared of speech.
struct Steps {
    bits: Vec<u8>,
    states: usize,
}

impl Steps {
    fn new(frames: usize, states: usize) -> Steps {
        Steps {
            bits: vec![0; (frames * states).div_ceil(4)],
            states,
        }
    }

    /// Each `(t, s)` is set once, over the zeroed start.
    fn set(&mut self, t: usize, s: usize, step: u8) {
        let i = t * self.states + s;
        self.bits[i / 4] |= step << (2 * (i % 4));
    }

    fn get(&self, t: usize, s: usize) -> usize {
        let i = t * self.states + s;
        usize::from((self.bits[i / 4] >> (2 * (i % 4))) & 3)
    }
}
