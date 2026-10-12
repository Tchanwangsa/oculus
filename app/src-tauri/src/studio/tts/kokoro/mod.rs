//! Kokoro run inside the app on ONNX Runtime: the `kokoro` voice engine.
//! A scene's text is read to phonemes (`reader`, over `studio::voice::g2p`), cut into
//! inputs under the model's length (`chunk`), each synthesised and checked
//! for silence (`synth`), and the samples joined into one 24 kHz WAV, the
//! shape a TTS server's clip has. The model and voices are the
//! `voice-kokoro` pack (`model`).

mod chunk;
mod model;
mod reader;
mod speaker;
mod synth;
#[cfg(test)]
mod tests;
pub mod vocab;

pub use chunk::{chunks, Chunk, MAX_TOKENS};
pub use model::{
    cancel, delete, dir, download, downloading, packs, status, voice_file, voices, MODEL_FILE,
    MODEL_ID, PACK, VOICES, VOICE_BYTES,
};
pub use reader::{LexiconFiles, Reader};
pub use speaker::{KokoroSpeaker, Packs};
pub use synth::SAMPLE_RATE;
