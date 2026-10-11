//! Narration audio in: a TTS server's WAV decoded to mono f32, resampled to
//! the aligner's 16 kHz (or a render's 48 kHz), and checked for digital
//! silence. Pure Rust (hound, rubato), no ffmpeg spawn; WAV is the only
//! format accepted.

mod decode;
mod resample;
mod silence;
#[cfg(test)]
mod tests;

pub use decode::{decode_wav, Decoded};
pub use resample::{resample, resample_to_16k, ALIGN_RATE};
pub use silence::{is_silent, silence, Silence, SILENCE_RMS};
