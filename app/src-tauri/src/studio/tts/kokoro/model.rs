//! The `voice-kokoro` pack: Kokoro-82M's int8 ONNX model and the US voices'
//! style files, downloaded on request to `<app data>/models/voice/` beside
//! the `g2p-en-us` pack, never bundled.

use std::path::{Path, PathBuf};

use crate::runtime::onnx::{Pack, PackFile};

/// Names the model in a narration cache key, so changing it re-narrates.
pub const MODEL_ID: &str = "kokoro-82m-v1.0-onnx-int8@1939ad2a";

pub const MODEL_FILE: &str = "kokoro-v1.0-int8.onnx";

/// The voices the pack holds. A voice is `<id>.bin` beside the model.
pub const VOICES: [&str; 2] = ["af_heart", "am_michael"];

/// A voice file's size: [`STYLE_ROWS`] rows of [`STYLE_WIDTH`] f32s.
pub const VOICE_BYTES: u64 = (STYLE_ROWS * STYLE_WIDTH * 4) as u64;
/// A voice file's rows; row `n` styles an `n`-token input, so an input
/// holds at most `STYLE_ROWS - 1` tokens.
pub const STYLE_ROWS: usize = 510;
pub const STYLE_WIDTH: usize = 256;

/// onnx-community/Kokoro-82M-v1.0-ONNX (Apache-2.0), pinned to a commit.
/// int8: 92 MB and word-perfect; fp16 and q4f16 come out silent.
pub const PACK: Pack = Pack {
    id: "voice-kokoro",
    files: &[
        PackFile {
            file: MODEL_FILE,
            url: "https://huggingface.co/onnx-community/Kokoro-82M-v1.0-ONNX/resolve/1939ad2a8e416c0acfeecc08a694d14ef25f2231/onnx/model_quantized.onnx",
            bytes: 92_361_116,
            sha256: "fbae9257e1e05ffc727e951ef9b9c98418e6d79f1c9b6b13bd59f5c9028a1478",
        },
        PackFile {
            file: "af_heart.bin",
            url: "https://huggingface.co/onnx-community/Kokoro-82M-v1.0-ONNX/resolve/1939ad2a8e416c0acfeecc08a694d14ef25f2231/voices/af_heart.bin",
            bytes: VOICE_BYTES,
            sha256: "d583ccff3cdca2f7fae535cb998ac07e9fcb90f09737b9a41fa2734ec44a8f0b",
        },
        PackFile {
            file: "am_michael.bin",
            url: "https://huggingface.co/onnx-community/Kokoro-82M-v1.0-ONNX/resolve/1939ad2a8e416c0acfeecc08a694d14ef25f2231/voices/am_michael.bin",
            bytes: VOICE_BYTES,
            sha256: "1d1f21dd8da39c30705cd4c75d039d265e9bc4a2a93ed09bc9e1b1225eb95ba1",
        },
    ],
};

/// Where the pack is kept.
pub fn dir() -> PathBuf {
    crate::studio::voice::g2p::guess::dir()
}

/// `voice`'s style file in `dir`, when it is there at its size.
pub fn voice_file(dir: &Path, voice: &str) -> Option<PathBuf> {
    let path = dir.join(format!("{voice}.bin"));
    std::fs::metadata(&path)
        .is_ok_and(|m| m.is_file() && m.len() == VOICE_BYTES)
        .then_some(path)
}
