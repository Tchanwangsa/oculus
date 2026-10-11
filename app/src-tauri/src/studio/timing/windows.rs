//! Where a long clip is cut for the model. wav2vec2's self-attention makes
//! its memory grow with the square of the audio it sees, so a clip over
//! [`Windows::single_pass`] frames runs as several windows, each with
//! context audio on both sides, and only each window's own frames are kept.
//! The Viterbi then runs once over the stitched emissions. Numbers and
//! measurements: docs/studio.md, "Narration timing".

use std::ops::Range;

/// Samples per emission frame at 16 kHz (the feature extractor's stride).
pub(super) const FRAME_SAMPLES: usize = 320;

/// Samples one frame sees: the feature extractor's receptive field.
const RECEPTIVE_SAMPLES: usize = 400;

/// Frames the model emits for `samples` samples: one per stride whose
/// receptive field fits entirely.
pub(super) fn frames(samples: usize) -> usize {
    if samples < RECEPTIVE_SAMPLES {
        0
    } else {
        (samples - RECEPTIVE_SAMPLES) / FRAME_SAMPLES + 1
    }
}

/// The samples frames `run` are computed from.
pub(super) fn samples(run: &Range<usize>) -> Range<usize> {
    run.start * FRAME_SAMPLES..(run.end - 1) * FRAME_SAMPLES + RECEPTIVE_SAMPLES
}

/// How a clip is split, all in frames (20 ms).
#[derive(Debug, Clone, PartialEq)]
pub(super) struct Windows {
    /// A clip of at most this many frames runs in one pass, as a whole.
    pub single_pass: usize,
    /// Longer clips are cut into `ceil(frames / target)` windows of even
    /// length, so none is over this before its cuts move.
    pub target: usize,
    /// Each cut lands on the quietest frame within this many of its target.
    pub search: usize,
    /// Audio run on each side of a window and then dropped.
    pub context: usize,
}

impl Windows {
    /// 40 s in one pass; past that windows of at most 30 s, each cut within
    /// 3 s of its even position, with 2 s of context. No model run sees more
    /// than 40 s (30 + 2 x 3 + 2 x 2), so peak memory stays at a 40 s single
    /// pass's at any clip length.
    pub const DEFAULT: Windows = Windows {
        single_pass: 2000,
        target: 1500,
        search: 150,
        context: 100,
    };

    /// Any clip in one pass, however long: the reference chunking is
    /// measured against.
    #[cfg(test)]
    pub fn whole() -> Windows {
        Windows {
            single_pass: usize::MAX,
            ..Windows::DEFAULT
        }
    }

    /// Each window's own frames, in order, covering `frames(pcm.len())`
    /// exactly once. One window when the clip fits in a single pass.
    /// Otherwise `ceil(frames / target)` windows of even length, each cut
    /// moved to the quietest frame (lowest energy, ties to the nearest) within
    /// `search` of its even position, never leaving a window under half of
    /// `target`.
    pub fn plan(&self, pcm: &[f32]) -> Vec<Range<usize>> {
        let total = frames(pcm.len());
        let whole = 0..total;
        if total <= self.single_pass {
            return vec![whole];
        }
        let count = total.div_ceil(self.target);
        let min = self.target / 2;
        let mut out = Vec::with_capacity(count);
        let mut start = 0;
        for i in 1..count {
            let goal = i * total / count;
            let lo = goal.saturating_sub(self.search).max(start + min);
            let hi = (goal + self.search).min(total.saturating_sub(min));
            let cut = if lo > hi {
                goal
            } else {
                quietest(pcm, lo..hi + 1, goal)
            };
            out.push(start..cut);
            start = cut;
        }
        out.push(start..total);
        out
    }

    /// The frames run for `own`: `context` more on each side, within the
    /// clip's `total`.
    pub fn run(&self, own: &Range<usize>, total: usize) -> Range<usize> {
        own.start.saturating_sub(self.context)..(own.end + self.context).min(total)
    }
}

/// The frame in `range` whose 320 samples carry the least energy; ties go to
/// the one nearest `goal`, then the earlier.
fn quietest(pcm: &[f32], range: Range<usize>, goal: usize) -> usize {
    let energy = |f: usize| {
        let from = (f * FRAME_SAMPLES).min(pcm.len());
        let to = ((f + 1) * FRAME_SAMPLES).min(pcm.len());
        pcm[from..to]
            .iter()
            .map(|&s| (s as f64) * (s as f64))
            .sum::<f64>()
    };
    range
        .map(|f| (energy(f), f.abs_diff(goal), f))
        .min_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)).then(a.2.cmp(&b.2)))
        .map(|(_, _, f)| f)
        .expect("the search range is not empty")
}
