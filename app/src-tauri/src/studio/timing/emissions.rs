//! Audio to per-frame label log-probabilities through the ONNX model.

use std::path::Path;

use ort::session::Session;
use ort::value::Tensor;

use super::targets::VOCAB;
use super::windows::{frames, samples, Windows};
use crate::studio::NarrationError;

/// `[frames, VOCAB]` log-probs for the whole of mono 16 kHz `pcm` from the
/// model at `model`, one window of `windows`' plan at a time. A clip that
/// fits in one pass runs as a whole; otherwise each window runs with its
/// context, normalised on its own, and keeps only its own frames.
///
/// Each run opens its own session and drops it: a session reused across
/// runs keeps growing, and a load costs only about 0.2 s.
pub(super) fn emissions(
    model: &Path,
    pcm: &[f32],
    windows: &Windows,
) -> Result<Vec<f32>, NarrationError> {
    let plan = windows.plan(pcm);
    if plan.len() == 1 {
        return Ok(log_softmax(&logits(&mut session(model)?, normalise(pcm))?));
    }
    let total = frames(pcm.len());
    let mut out = Vec::with_capacity(total * VOCAB);
    for own in &plan {
        let run = windows.run(own, total);
        let audio = &pcm[samples(&run)];
        let logits = logits(&mut session(model)?, normalise(audio))?;
        if logits.len() != run.len() * VOCAB {
            return Err(NarrationError::Onnx(format!(
                "the model gave {} frames for {} samples, expected {}",
                logits.len() / VOCAB,
                audio.len(),
                run.len()
            )));
        }
        let keep = (own.start - run.start) * VOCAB..(own.end - run.start) * VOCAB;
        out.extend(log_softmax(&logits[keep]));
    }
    Ok(out)
}

fn session(model: &Path) -> Result<Session, NarrationError> {
    crate::runtime::onnx::session(model).map_err(NarrationError::Onnx)
}

/// Zero mean, unit variance over `x` (a clip or one window of it), as
/// wav2vec2's feature extractor does (`do_normalize`); the exported graph
/// does not. Computed in
/// f64 and rounded once, which matched Python frame for frame.
pub(super) fn normalise(x: &[f32]) -> Vec<f32> {
    let n = x.len() as f64;
    let mean = x.iter().map(|&v| v as f64).sum::<f64>() / n;
    let var = x.iter().map(|&v| (v as f64 - mean).powi(2)).sum::<f64>() / n;
    let sd = (var + 1e-7).sqrt();
    x.iter().map(|&v| ((v as f64 - mean) / sd) as f32).collect()
}

/// The model's `[frames, 32]` logits for normalised 16 kHz `input`, flattened.
pub(super) fn logits(session: &mut Session, input: Vec<f32>) -> Result<Vec<f32>, NarrationError> {
    let fail = |e: ort::Error| NarrationError::Onnx(e.to_string());
    let n = input.len();
    let tensor = Tensor::from_array(([1usize, n], input)).map_err(fail)?;
    let outputs = session
        .run(ort::inputs!["input_values" => tensor])
        .map_err(fail)?;
    let (shape, data) = outputs["logits"]
        .try_extract_tensor::<f32>()
        .map_err(fail)?;
    if shape.len() != 3 || shape[2] as usize != VOCAB {
        return Err(NarrationError::Onnx(format!(
            "unexpected logits shape {shape:?}"
        )));
    }
    Ok(data.to_vec())
}

/// Row-wise log-softmax over `[t, VOCAB]` logits, in f32 like numpy's.
pub(super) fn log_softmax(logits: &[f32]) -> Vec<f32> {
    let mut out = Vec::with_capacity(logits.len());
    for row in logits.chunks_exact(VOCAB) {
        let max = row.iter().copied().fold(f32::NEG_INFINITY, f32::max);
        let lse = row.iter().map(|x| (x - max).exp()).sum::<f32>().ln();
        out.extend(row.iter().map(|x| x - max - lse));
    }
    out
}
