//! One scene's clip: the section said in one go and timed in one pass, as
//! without pauses, then each `[pause]` spliced in as exact silence at the
//! quietest moment between the words either side of it, the times after it
//! moved on by its length. Saying the words whole keeps the voice's pace:
//! Kokoro paces an input by its length, so parts said apart came out up to
//! 11% slower than the same words said whole. A server is asked once, too.
//! A clip with no pause is the speaker's WAV byte for byte.

use std::time::Instant;

use studio_core::{ManifestGap, ManifestPause, ManifestWord};

use super::{Aligner, Speaker, Timings};
use crate::studio::audio::{encode_wav, resample_to_16k};
use crate::studio::timing::{Gap, WordTime};
use crate::studio::tts::Speech;

/// A made clip and what its manifest entry records.
pub(super) struct Made {
    pub wav: Vec<u8>,
    pub duration: f64,
    pub words: Vec<ManifestWord>,
    pub gaps: Vec<ManifestGap>,
    pub pauses: Vec<ManifestPause>,
    pub timings: Timings,
}

/// The clip of `spoken` with `pauses`, each `(words before it, seconds)`
/// in script order.
pub(super) fn make(
    spoken: &str,
    pauses: &[(usize, f64)],
    speaker: &dyn Speaker,
    aligner: &dyn Aligner,
) -> Result<Made, String> {
    let started = Instant::now();
    let Speech { wav, decoded } = speaker.speak(spoken)?;
    let synth_ms = started.elapsed().as_millis() as u64;
    let started = Instant::now();
    let rate = decoded.sample_rate;
    let pcm = resample_to_16k(&decoded.samples, rate).map_err(|e| e.to_string())?;
    let script: Vec<String> = spoken.split_whitespace().map(str::to_owned).collect();
    let alignment = aligner.align(&pcm, &script).map_err(|e| e.to_string())?;
    drop(pcm);
    let timings = Timings {
        synth_ms,
        align_ms: started.elapsed().as_millis() as u64,
    };
    let seconds = |n: usize| n as f64 / f64::from(rate);
    let spliced = splice(&decoded.samples, rate, &alignment.words, pauses);
    // A word after a pause moves by it; a gap by the pauses cut before it.
    let words = alignment.words.iter().map(|w| {
        let after = spliced.cuts.iter().zip(pauses);
        let n: usize = after
            .filter(|(_, (k, _))| w.index >= *k)
            .map(|(c, _)| c.1)
            .sum();
        word(w, seconds(n))
    });
    let gaps = alignment.gaps.iter().map(|g| {
        let at = (g.start * f64::from(rate)).round() as usize;
        let n: usize = spliced.cuts.iter().filter(|c| c.0 <= at).map(|c| c.1).sum();
        gap(g, seconds(n))
    });
    let (words, gaps) = (words.collect(), gaps.collect());
    if pauses.is_empty() {
        return Ok(Made {
            wav,
            duration: decoded.duration,
            words,
            gaps,
            pauses: Vec::new(),
            timings,
        });
    }
    Ok(Made {
        wav: encode_wav(&spliced.samples, rate)?,
        duration: seconds(spliced.samples.len()),
        words,
        gaps,
        pauses: spliced.pauses,
        timings,
    })
}

fn word(w: &WordTime, shift: f64) -> ManifestWord {
    ManifestWord {
        text: w.text.clone(),
        start: w.start + shift,
        end: w.end + shift,
        confidence: w.confidence,
    }
}

fn gap(g: &Gap, shift: f64) -> ManifestGap {
    ManifestGap {
        after: g.after,
        start: g.start + shift,
        end: g.end + shift,
        heard: g.heard.clone(),
    }
}

/// A clip with its pauses in.
struct Spliced {
    samples: Vec<f32>,
    /// Per pause: the sample of the clip as spoken it went before, and
    /// how many zeros it is.
    cuts: Vec<(usize, usize)>,
    pauses: Vec<ManifestPause>,
}

/// `samples` with each pause's zeros inserted at its `cut`. Without pauses
/// it copies nothing.
fn splice(samples: &[f32], rate: u32, words: &[WordTime], pauses: &[(usize, f64)]) -> Spliced {
    let mut spliced = Spliced {
        samples: Vec::new(),
        cuts: Vec::with_capacity(pauses.len()),
        pauses: Vec::with_capacity(pauses.len()),
    };
    if pauses.is_empty() {
        return spliced;
    }
    let out = &mut spliced.samples;
    let mut from = 0;
    for &(k, seconds) in pauses {
        let c = cut(samples, rate, words, k).max(from);
        out.extend_from_slice(&samples[from..c]);
        from = c;
        let n = (seconds * f64::from(rate)).round() as usize;
        let start = out.len() as f64 / f64::from(rate);
        out.resize(out.len() + n, 0.0);
        spliced.pauses.push(ManifestPause {
            after: words.iter().rposition(|w| w.index < k),
            start,
            end: out.len() as f64 / f64::from(rate),
        });
        spliced.cuts.push((c, n));
    }
    out.extend_from_slice(&samples[from..]);
    spliced
}

/// Half the span whose energy places a cut: 5 ms either side.
const CUT_HALF_SECONDS: f64 = 0.005;

/// The sample a pause after `k` script words goes before: the clip's start
/// before the first word, its end after the last, else the quietest 10 ms
/// between the last word before it ending and the next one starting (ties
/// to the middle), so the cut lands in the silence the voice left there.
pub(super) fn cut(samples: &[f32], rate: u32, words: &[WordTime], k: usize) -> usize {
    let at = |t: f64| ((t * f64::from(rate)).round().max(0.0) as usize).min(samples.len());
    let before = words.iter().filter(|w| w.index < k).map(|w| w.end);
    let after = words.iter().filter(|w| w.index >= k).map(|w| w.start);
    let (a, b) = match (before.reduce(f64::max), after.reduce(f64::min)) {
        (None, _) => return 0,
        (_, None) => return samples.len(),
        (Some(a), Some(b)) => (at(a), at(b)),
    };
    if b <= a {
        return a;
    }
    let half = (CUT_HALF_SECONDS * f64::from(rate)).round() as usize;
    let energy = |c: usize| -> f64 {
        let span = &samples[c.saturating_sub(half)..(c + half).min(samples.len())];
        span.iter().map(|&s| f64::from(s).powi(2)).sum()
    };
    let middle = (a + b) / 2;
    (a..=b)
        .step_by((half / 2).max(1))
        .map(|c| (energy(c), c.abs_diff(middle), c))
        .min_by(|x, y| x.0.total_cmp(&y.0).then(x.1.cmp(&y.1)))
        .map_or(a, |(_, _, c)| c)
}
