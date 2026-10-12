//! The aligner's model: wav2vec2-base-960h as ONNX, downloaded on request to
//! `<app data>/models/align/`, never bundled.

use std::path::{Path, PathBuf};

pub use crate::runtime::onnx::Status;
use crate::runtime::onnx::{self, Pack, PackFile};
use crate::studio::NarrationError;

/// Names the model in a narration cache key, so changing the model
/// re-times every scene.
pub const MODEL_ID: &str = "wav2vec2-base-960h-fp32@a19f851b";

/// Xenova's fp32 export of facebook/wav2vec2-base-960h (Apache-2.0), pinned
/// to a commit. fp32, not int8: int8 is slower on ORT 1.28 and its
/// confidences are unstable.
pub const PACK: Pack = Pack {
    id: "align-wav2vec2",
    files: &[PackFile {
        file: "wav2vec2-base-960h.onnx",
        url: "https://huggingface.co/Xenova/wav2vec2-base-960h/resolve/a19f851b3d42865797e410752b4c570c871e4825/onnx/model.onnx",
        bytes: 377_887_594,
        sha256: "e46614273f03ff4b87923a965e417fa72004825522cb007c9c25633b8475490d",
    }],
};

/// Where the model is kept.
pub fn dir() -> PathBuf {
    crate::library::paths::data_dir()
        .join("models")
        .join("align")
}

/// Whether the model is in `dir`. Local only.
pub fn status(dir: &Path) -> Status {
    onnx::status(dir, &PACK)
}

/// The model file, when it is downloaded.
pub fn model_file(dir: &Path) -> Option<PathBuf> {
    onnx::is_present(dir, &PACK).then(|| dir.join(PACK.files[0].file))
}

/// Overrides the downloaded model with a file elsewhere, for development
/// and tests (the parity tests read it too), so they need not download
/// 378 MB into the app's data folder.
pub const MODEL_ENV: &str = "STUDIO_ALIGN_MODEL";

/// The model narration aligns with: [`MODEL_ENV`] when set, else the
/// downloaded one.
pub fn resolve_model() -> Result<PathBuf, NarrationError> {
    if let Some(path) = std::env::var_os(MODEL_ENV).filter(|p| !p.is_empty()) {
        let path = PathBuf::from(path);
        if !path.is_file() {
            return Err(NarrationError::Onnx(format!(
                "{MODEL_ENV} names {}, which is not a file",
                path.display()
            )));
        }
        return Ok(path);
    }
    model_file(&dir()).ok_or(NarrationError::ModelMissing)
}

/// Download the model into `dir`, reporting `(received, total)` bytes. Ends
/// in `Err(CANCELLED)` when [`cancel`]led or when `stop` returns true.
pub fn download(
    dir: &Path,
    progress: impl Fn(u64, u64),
    stop: impl Fn() -> bool,
) -> Result<(), String> {
    onnx::download(dir, &PACK, progress, stop)
}

/// Ask an in-flight download to stop; false when none runs.
pub fn cancel() -> bool {
    onnx::cancel(PACK.id)
}

/// Delete the model and any partial download; returns the bytes freed.
pub fn delete(dir: &Path) -> Result<u64, String> {
    onnx::delete(dir, &PACK)
}
