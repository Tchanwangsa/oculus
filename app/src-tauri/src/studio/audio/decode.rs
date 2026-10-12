//! WAV bytes to mono f32 samples.

use std::io::Cursor;

use crate::studio::NarrationError;

pub struct Decoded {
    /// Mono, at `sample_rate`, in [-1, 1].
    pub samples: Vec<f32>,
    pub sample_rate: u32,
    /// From the source sample count, so exact.
    pub duration: f64,
}

/// Decode a WAV of integer (8 to 32 bit) or float samples at any rate,
/// averaging the channels to mono.
pub fn decode_wav(bytes: &[u8]) -> Result<Decoded, NarrationError> {
    let fail = |e: hound::Error| NarrationError::Decode(e.to_string());
    let mut reader = hound::WavReader::new(Cursor::new(bytes)).map_err(fail)?;
    let spec = reader.spec();
    if spec.channels == 0 || spec.sample_rate == 0 {
        return Err(NarrationError::Decode(format!(
            "the WAV header says {} channels at {} Hz",
            spec.channels, spec.sample_rate
        )));
    }
    let interleaved: Vec<f32> = match spec.sample_format {
        hound::SampleFormat::Float => reader
            .samples::<f32>()
            .collect::<Result<_, _>>()
            .map_err(fail)?,
        hound::SampleFormat::Int => {
            let scale = 1.0 / (1u64 << (spec.bits_per_sample - 1)) as f32;
            reader
                .samples::<i32>()
                .map(|s| s.map(|v| v as f32 * scale))
                .collect::<Result<_, _>>()
                .map_err(fail)?
        }
    };
    let samples = downmix(&interleaved, spec.channels as usize);
    Ok(Decoded {
        duration: samples.len() as f64 / spec.sample_rate as f64,
        sample_rate: spec.sample_rate,
        samples,
    })
}

fn downmix(interleaved: &[f32], channels: usize) -> Vec<f32> {
    if channels == 1 {
        return interleaved.to_vec();
    }
    interleaved
        .chunks_exact(channels)
        .map(|frame| frame.iter().sum::<f32>() / channels as f32)
        .collect()
}
