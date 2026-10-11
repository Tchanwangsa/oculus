//! A narration clip and its script in, each script word's time out.

use std::path::Path;

use serde::{Deserialize, Serialize};

use super::emissions::{log_softmax, logits, normalise};
use super::gaps::{unscripted, Heard};
use super::model::resolve_model;
use super::targets::targets;
use super::viterbi::{force_align, Span};
use crate::studio::audio::ALIGN_RATE;
use crate::studio::NarrationError;
use studio_core::MAX_NARRATION_SECS;

/// One emission frame of wav2vec2: 320 samples at 16 kHz.
pub const FRAME_SECONDS: f64 = 0.02;

/// Added to every word start: CTC fires a little after a word's onset. -65 ms
/// was measured against PyTorch wav2vec2 on Kokoro clips; the fp32 ONNX model
/// is frame-identical to it. Ends take no bias.
pub const START_BIAS: f64 = -0.065;

/// A gap with this many letter frames is speech the script lacks (clean
/// clips reach 1; a dropped word gave 7 and 13).
pub const GAP_LETTER_FRAMES: usize = 4;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WordTime {
    /// Index of the word in the script words passed to [`align`].
    pub index: usize,
    /// The script's own text for it, punctuation and case kept.
    pub text: String,
    /// Seconds from the clip's start.
    pub start: f64,
    pub end: f64,
    /// Mean log-prob of the frames on the word's letters; 0 is certain.
    /// Below `studio_core::LOW_CONFIDENCE` is a word to check.
    pub confidence: f64,
}

/// Speech between two words that no script word accounts for.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Gap {
    /// Index in [`Alignment::words`] of the word before the gap, as the
    /// manifest's `after`; None for speech before the first word.
    pub after: Option<usize>,
    pub start: f64,
    pub end: f64,
    /// What the model heard there, upper case, e.g. `THICK`.
    pub heard: String,
}

/// Word times for the script, and the gaps of [`GAP_LETTER_FRAMES`] or more.
///
/// A script word with no letter in it (a dash, `&`) is not in `words`: the
/// model cannot spell it, and a made-up time would read as a measured one.
/// `index` maps every word back to the script, so a caller sees which ones
/// were left out. If the TTS did say such a word aloud, it shows up as a gap.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Alignment {
    pub words: Vec<WordTime>,
    pub gaps: Vec<Gap>,
}

/// Time `script_words` against mono 16 kHz `pcm` with the downloaded model
/// (or `STUDIO_ALIGN_MODEL`'s; see `resolve_model`).
pub fn align(pcm: &[f32], script_words: &[String]) -> Result<Alignment, NarrationError> {
    let model = resolve_model()?;
    align_with(&model, pcm, script_words)
}

/// [`align`] with the model at `model`. Opens a session for this clip and
/// drops it after: a reused session's memory grows from run to run. A clip
/// over `MAX_NARRATION_SECS` is refused, since memory grows with the square
/// of its length (2.2 GB at 57 s).
pub fn align_with(
    model: &Path,
    pcm: &[f32],
    script_words: &[String],
) -> Result<Alignment, NarrationError> {
    let targets = targets(script_words);
    if targets.words.is_empty() {
        return Err(NarrationError::EmptyScript);
    }
    let seconds = pcm.len() as f64 / ALIGN_RATE as f64;
    if seconds > MAX_NARRATION_SECS {
        return Err(NarrationError::TooLong { seconds });
    }
    let logits = {
        let mut session = crate::runtime::onnx::session(model).map_err(NarrationError::Onnx)?;
        logits(&mut session, normalise(pcm))?
    };
    let em = log_softmax(&logits);
    drop(logits);
    let spans = force_align(&em, &targets.words)?;
    let heard = unscripted(&em, &spans);
    Ok(assemble(&spans, &heard, &targets.index, script_words))
}

/// Seconds from frames, the start bias applied, and spans mapped back to
/// script words (`index[k]` is span `k`'s script index).
pub(super) fn assemble(
    spans: &[Span],
    heard: &[Heard],
    index: &[usize],
    script_words: &[String],
) -> Alignment {
    let words = spans
        .iter()
        .zip(index)
        .map(|(span, &i)| WordTime {
            index: i,
            text: script_words[i].clone(),
            start: (span.start_frame as f64 * FRAME_SECONDS + START_BIAS).max(0.0),
            end: (span.end_frame + 1) as f64 * FRAME_SECONDS,
            confidence: span.mean_logp,
        })
        .collect();
    let gaps = heard
        .iter()
        .filter(|h| h.letter_frames >= GAP_LETTER_FRAMES)
        .map(|h| Gap {
            after: h.next.checked_sub(1),
            start: h.start_frame as f64 * FRAME_SECONDS,
            end: h.end_frame as f64 * FRAME_SECONDS,
            heard: h.heard.clone(),
        })
        .collect();
    Alignment { words, gaps }
}
