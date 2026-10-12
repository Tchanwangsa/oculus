//! The `voice-kokoro` pack: Kokoro-82M's int8 ONNX model, downloaded on
//! request to `<app data>/models/voice/` beside the `g2p-en-us` pack and the
//! voices' style files (`voices`), never bundled.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};

use crate::runtime::onnx::{self, Pack, PackFile, Status, CANCELLED};
use crate::studio::voice::g2p::guess;

/// Names the model in a narration cache key, so changing it re-narrates.
pub const MODEL_ID: &str = "kokoro-82m-v1.0-onnx-int8@1939ad2a";

pub const MODEL_FILE: &str = "kokoro-v1.0-int8.onnx";

/// A voice file's size: [`STYLE_ROWS`] rows of [`STYLE_WIDTH`] f32s.
pub const VOICE_BYTES: u64 = (STYLE_ROWS * STYLE_WIDTH * 4) as u64;
/// A voice file's rows; row `n` styles an `n`-token input, so an input
/// holds at most `STYLE_ROWS - 1` tokens.
pub const STYLE_ROWS: usize = 510;
pub const STYLE_WIDTH: usize = 256;

/// onnx-community/Kokoro-82M-v1.0-ONNX's model (Apache-2.0), pinned to a
/// commit. int8: 92 MB and word-perfect; fp16 and q4f16 come out silent.
pub const PACK: Pack = Pack {
    id: "voice-kokoro",
    files: &[PackFile {
        file: MODEL_FILE,
        url: "https://huggingface.co/onnx-community/Kokoro-82M-v1.0-ONNX/resolve/1939ad2a8e416c0acfeecc08a694d14ef25f2231/onnx/model_quantized.onnx",
        bytes: 92_361_116,
        sha256: "fbae9257e1e05ffc727e951ef9b9c98418e6d79f1c9b6b13bd59f5c9028a1478",
    }],
};

/// Where the pack is kept.
pub fn dir() -> PathBuf {
    crate::studio::voice::g2p::guess::dir()
}

/// The packs Kokoro narration needs besides a voice, both kept in [`dir`]:
/// the model and the G2P.
pub fn packs() -> [&'static Pack; 2] {
    [&PACK, &guess::PACK]
}

/// Both packs together: present when both are in `dir`, downloading while
/// [`download`] runs. Local only.
pub fn status(dir: &Path) -> Status {
    if packs().iter().all(|p| onnx::is_present(dir, p)) {
        Status::Present
    } else if downloading() {
        Status::Downloading
    } else {
        Status::Missing
    }
}

/// Set while [`download`] runs; one at a time across both packs.
static RUNNING: AtomicBool = AtomicBool::new(false);
/// Set by [`cancel`]: checked between and inside the packs' downloads, so a
/// cancel that lands between the two packs still stops the second.
static STOP: AtomicBool = AtomicBool::new(false);

/// Clears [`RUNNING`] however the download ends.
struct Running;

impl Drop for Running {
    fn drop(&mut self) {
        RUNNING.store(false, Ordering::SeqCst);
    }
}

/// Download both packs into `dir`, the files already there skipped,
/// reporting `(received, total)` bytes over both. Ends in
/// `Err(CANCELLED)` when [`cancel`]led or when `stop` returns true.
pub fn download(
    dir: &Path,
    progress: impl Fn(u64, u64),
    stop: impl Fn() -> bool,
) -> Result<(), String> {
    if RUNNING.swap(true, Ordering::SeqCst) {
        return Err("Kokoro's packs are already downloading".into());
    }
    let _running = Running;
    STOP.store(false, Ordering::SeqCst);
    let stop = || stop() || STOP.load(Ordering::SeqCst);
    let total: u64 = packs().iter().map(|p| p.bytes()).sum();
    let mut before = 0;
    for pack in packs() {
        if stop() {
            return Err(CANCELLED.into());
        }
        onnx::download(
            dir,
            pack,
            |received, _| progress(before + received, total),
            stop,
        )?;
        before += pack.bytes();
    }
    Ok(())
}

/// A [`download`] is in flight in this process.
pub fn downloading() -> bool {
    RUNNING.load(Ordering::SeqCst)
}

/// Ask the in-flight [`download`] to stop; false when none runs.
pub fn cancel() -> bool {
    if !downloading() {
        return false;
    }
    STOP.store(true, Ordering::SeqCst);
    for pack in packs() {
        onnx::cancel(pack.id);
    }
    true
}

/// Delete both packs and any partial download, stopping a download first;
/// every voice's file stays. Returns the bytes freed.
pub fn delete(dir: &Path) -> Result<u64, String> {
    cancel();
    let mut freed = 0;
    for pack in packs() {
        freed += onnx::delete(dir, pack)?;
    }
    Ok(freed)
}
