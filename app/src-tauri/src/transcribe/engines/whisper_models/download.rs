//! Downloading, cancelling and deleting a model file.

use std::collections::HashMap;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use super::models::{find, Model, VAD_FILE, VAD_URL};

/// Model ids with a download in flight, each with its cancel flag.
static DOWNLOADS: Mutex<Option<HashMap<String, Arc<AtomicBool>>>> = Mutex::new(None);

pub(super) fn in_flight() -> Vec<String> {
    DOWNLOADS
        .lock()
        .unwrap()
        .as_ref()
        .map(|d| d.keys().cloned().collect())
        .unwrap_or_default()
}

/// One download's place in [`DOWNLOADS`], released however it ends so a
/// late cancel cannot poison a retry.
pub(super) struct Claim(pub(super) String, pub(super) Arc<AtomicBool>);

impl Claim {
    pub(super) fn take(id: &str) -> Result<Self, String> {
        let mut held = DOWNLOADS.lock().unwrap();
        let held = held.get_or_insert_with(HashMap::new);
        if held.contains_key(id) {
            return Err("that model is already downloading".into());
        }
        let flag = Arc::new(AtomicBool::new(false));
        held.insert(id.to_string(), Arc::clone(&flag));
        Ok(Self(id.to_string(), flag))
    }
}

impl Drop for Claim {
    fn drop(&mut self) {
        if let Some(held) = DOWNLOADS.lock().unwrap().as_mut() {
            held.remove(&self.0);
        }
    }
}

/// Ask an in-flight download to stop; false when none runs for `id`.
pub fn cancel(id: &str) -> bool {
    match DOWNLOADS.lock().unwrap().as_ref().and_then(|d| d.get(id)) {
        Some(flag) => {
            flag.store(true, Ordering::Relaxed);
            true
        }
        None => false,
    }
}

/// Download `model` into `dir` — and the VAD model first, if it is missing —
/// reporting `(received, total)` bytes of the model as it goes. Ends in
/// `Err(CANCELLED)` when [`cancel`]led.
pub fn download(dir: &Path, model: &Model, progress: impl Fn(u64, u64)) -> Result<PathBuf, String> {
    download_from(dir, model, &model.url(), VAD_URL, progress)
}

pub use crate::sources::echo360::CANCELLED;

pub(super) fn download_from(
    dir: &Path,
    model: &Model,
    model_url: &str,
    vad_url: &str,
    progress: impl Fn(u64, u64),
) -> Result<PathBuf, String> {
    let claim = Claim::take(model.id)?;
    let cancelled = || claim.1.load(Ordering::Relaxed);
    std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let vad = dir.join(VAD_FILE);
    if !vad.is_file() {
        fetch(vad_url, &vad, 0, &|_, _| {}, &cancelled).map_err(|e| {
            if e == CANCELLED {
                e
            } else {
                format!("the voice-detection model: {e}")
            }
        })?;
    }
    let dest = dir.join(model.file);
    fetch(model_url, &dest, model.bytes, &progress, &cancelled)?;
    Ok(dest)
}

/// No bytes for this long is a stalled connection, not a slow one; the whole
/// download has no deadline.
const STALL: Duration = Duration::from_secs(60);

/// Stream `url` to `<dest>.part`, then rename it into place. The part is
/// removed on any failure, so a retry starts clean.
fn fetch(
    url: &str,
    dest: &Path,
    expected: u64,
    progress: &dyn Fn(u64, u64),
    cancelled: &dyn Fn() -> bool,
) -> Result<u64, String> {
    let mut part = dest.as_os_str().to_os_string();
    part.push(".part");
    let part = PathBuf::from(part);
    let result = (|| {
        let agent = ureq::AgentBuilder::new().timeout_read(STALL).build();
        let response = agent.get(url).call().map_err(|e| match e {
            ureq::Error::Status(status, _) => format!("Hugging Face answered {status} for {url}"),
            e => format!("could not reach Hugging Face: {e}"),
        })?;
        let length = response
            .header("content-length")
            .and_then(|s| s.parse::<u64>().ok());
        let total = length.unwrap_or(expected);
        let mut reader = response.into_reader();
        let mut file =
            std::fs::File::create(&part).map_err(|e| format!("{}: {e}", part.display()))?;
        let mut buf = vec![0u8; 1 << 16];
        let mut received = 0u64;
        let mut last = Instant::now();
        progress(0, total);
        loop {
            if cancelled() {
                return Err(CANCELLED.to_string());
            }
            let n = reader
                .read(&mut buf)
                .map_err(|e| format!("the download broke off after {received} bytes: {e}"))?;
            if n == 0 {
                break;
            }
            file.write_all(&buf[..n])
                .map_err(|e| format!("{}: {e}", part.display()))?;
            received += n as u64;
            if last.elapsed() >= Duration::from_millis(150) {
                last = Instant::now();
                progress(received, total);
            }
        }
        if length.is_some_and(|length| received != length) {
            return Err(format!(
                "the download stopped at {received} of {total} bytes"
            ));
        }
        file.sync_all()
            .map_err(|e| format!("{}: {e}", part.display()))?;
        drop(file);
        std::fs::rename(&part, dest).map_err(|e| format!("{}: {e}", dest.display()))?;
        progress(received, total);
        Ok(received)
    })();
    if result.is_err() {
        std::fs::remove_file(&part).ok();
    }
    result
}

/// Delete `id`'s file (and any partial download, stopping it first); returns
/// the bytes freed. The VAD model stays: it is under 1 MB.
pub fn delete(dir: &Path, id: &str) -> Result<u64, String> {
    let model = find(id).ok_or_else(|| format!("no Whisper model called {id}"))?;
    cancel(id);
    let mut freed = 0;
    for name in [model.file.to_string(), format!("{}.part", model.file)] {
        let path = dir.join(name);
        if let Ok(meta) = std::fs::metadata(&path) {
            std::fs::remove_file(&path).map_err(|e| format!("{}: {e}", path.display()))?;
            freed += meta.len();
        }
    }
    Ok(freed)
}
