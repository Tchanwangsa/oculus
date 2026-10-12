//! Kokoro run inside the app on ONNX Runtime: the `kokoro` voice engine.
//! A scene's text is read to phonemes (`studio::voice::g2p`), cut into
//! inputs under the model's length (`chunk`), each synthesised and checked
//! for silence (`synth`), and the samples joined into one 24 kHz WAV, the
//! shape a TTS server's clip has. The model and voices are the
//! `voice-kokoro` pack (`model`).

mod chunk;
mod model;
mod speaker;
mod synth;
#[cfg(test)]
mod tests;
pub mod vocab;

pub use chunk::{chunks, Chunk, MAX_TOKENS};
pub use model::{dir, voice_file, MODEL_FILE, MODEL_ID, PACK, VOICES};
pub use speaker::{KokoroSpeaker, LexiconFiles, Packs};
pub use synth::SAMPLE_RATE;
