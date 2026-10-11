//! Mono audio from one rate to another: the aligner's 16 kHz, a render's
//! 48 kHz.

use rubato::audioadapter_buffers::direct::InterleavedSlice;
use rubato::{Fft, FixedSync, Resampler};

use crate::studio::NarrationError;

/// wav2vec2's input rate.
pub const ALIGN_RATE: u32 = 16_000;

/// Resample mono `samples` at `from` Hz to 16 kHz with rubato's FFT
/// resampler over the whole clip, as the alignment spike measured it
/// (within one 20 ms frame of ffmpeg's resampler on every word).
pub fn resample_to_16k(samples: &[f32], from: u32) -> Result<Vec<f32>, NarrationError> {
    resample(samples, from, ALIGN_RATE)
}

/// Resample mono `samples` from `from` Hz to `to` Hz with rubato's FFT
/// resampler over the whole clip.
pub fn resample(samples: &[f32], from: u32, to: u32) -> Result<Vec<f32>, NarrationError> {
    if from == to || samples.is_empty() {
        return Ok(samples.to_vec());
    }
    let fail = |e: &dyn std::fmt::Display| NarrationError::Decode(format!("resampling: {e}"));
    let mut resampler = Fft::<f32>::new(from as usize, to as usize, 1024, 1, FixedSync::Input)
        .map_err(|e| fail(&e))?;
    let input = InterleavedSlice::new(samples, 1, samples.len()).map_err(|e| fail(&e))?;
    Ok(resampler
        .process_all(&input, samples.len(), None)
        .map_err(|e| fail(&e))?
        .take_data())
}
