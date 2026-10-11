//! Audio to per-frame label log-probabilities through the ONNX model.

use ort::session::Session;
use ort::value::Tensor;

use super::targets::VOCAB;
use crate::studio::NarrationError;

/// Zero mean, unit variance over the whole clip, as wav2vec2's feature
/// extractor does (`do_normalize`); the exported graph does not. Computed in
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
