//! Kokoro's ONNX session: token ids, a style row of the voice and the speed
//! in, 24 kHz mono samples out. Inputs and voice layout as in the Rust
//! spike, which matched Python's ONNX Runtime sample for sample
//! (data/evals/studio-onnx-spike).

use std::io::Cursor;
use std::path::Path;

use ort::session::Session;
use ort::value::Tensor;

use super::chunk::Chunk;
use super::model::{STYLE_ROWS, STYLE_WIDTH};
use crate::runtime::onnx;
use crate::studio::audio::silence;

/// Kokoro's output rate.
pub const SAMPLE_RATE: u32 = 24_000;

/// One input to samples. [`Kokoro`] runs the model; tests stub it.
pub trait Synth {
    /// `ids` without the pads.
    fn run(&mut self, ids: &[i64]) -> Result<Vec<f32>, String>;
}

/// The model session and one voice's style table.
pub struct Kokoro {
    session: Session,
    /// `STYLE_ROWS` rows of `STYLE_WIDTH`.
    styles: Vec<f32>,
    speed: f32,
}

impl Kokoro {
    pub fn load(model: &Path, voice: &Path, speed: f64) -> Result<Self, String> {
        let bytes = std::fs::read(voice).map_err(|e| format!("{}: {e}", voice.display()))?;
        if bytes.len() != STYLE_ROWS * STYLE_WIDTH * 4 {
            return Err(format!(
                "{}: {} bytes, not a Kokoro voice ({} rows of {} f32s)",
                voice.display(),
                bytes.len(),
                STYLE_ROWS,
                STYLE_WIDTH
            ));
        }
        let styles = bytes
            .as_chunks::<4>()
            .0
            .iter()
            .map(|b| f32::from_le_bytes(*b))
            .collect();
        Ok(Self {
            session: onnx::session(model)?,
            styles,
            speed: speed as f32,
        })
    }
}

impl Synth for Kokoro {
    /// Pads `ids` with 0 at both ends and styles them with row `ids.len()`.
    fn run(&mut self, ids: &[i64]) -> Result<Vec<f32>, String> {
        let fail = |e: ort::Error| e.to_string();
        let row = ids.len();
        if row == 0 || row >= STYLE_ROWS {
            return Err(format!(
                "an input of {row} tokens; Kokoro takes 1 to {}",
                STYLE_ROWS - 1
            ));
        }
        let padded: Vec<i64> = std::iter::once(0)
            .chain(ids.iter().copied())
            .chain(std::iter::once(0))
            .collect();
        let style = self.styles[row * STYLE_WIDTH..(row + 1) * STYLE_WIDTH].to_vec();
        let outputs = self
            .session
            .run(ort::inputs![
                "input_ids" => Tensor::from_array(([1usize, padded.len()], padded)).map_err(fail)?,
                "style" => Tensor::from_array(([1usize, STYLE_WIDTH], style)).map_err(fail)?,
                "speed" => Tensor::from_array(([1usize], vec![self.speed])).map_err(fail)?,
            ])
            .map_err(fail)?;
        let (_, samples) = outputs["waveform"]
            .try_extract_tensor::<f32>()
            .map_err(fail)?;
        Ok(samples.to_vec())
    }
}

/// Every chunk through `synth`, joined with no gap. A chunk that comes out
/// digitally silent fails naming its words: int8 Kokoro never does, but a
/// broken model or voice file would, with no error of its own.
pub fn render(chunks: &[Chunk], synth: &mut dyn Synth) -> Result<Vec<f32>, String> {
    let mut audio = Vec::new();
    for (i, chunk) in chunks.iter().enumerate() {
        let samples = synth.run(&chunk.ids)?;
        if let Some(s) = silence(&samples, SAMPLE_RATE) {
            return Err(format!(
                "Kokoro made silence for part {} of {} ({s}): \"{}\"",
                i + 1,
                chunks.len(),
                chunk.words
            ));
        }
        audio.extend_from_slice(&samples);
    }
    Ok(audio)
}

/// `samples` as a 16-bit mono WAV at [`SAMPLE_RATE`], as a TTS server sends.
pub fn wav(samples: &[f32]) -> Result<Vec<u8>, String> {
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate: SAMPLE_RATE,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let fail = |e: hound::Error| format!("writing the clip: {e}");
    let mut cursor = Cursor::new(Vec::new());
    let mut writer = hound::WavWriter::new(&mut cursor, spec).map_err(fail)?;
    for &s in samples {
        let v = (s.clamp(-1.0, 1.0) * f32::from(i16::MAX)).round() as i16;
        writer.write_sample(v).map_err(fail)?;
    }
    writer.finalize().map_err(fail)?;
    Ok(cursor.into_inner())
}
