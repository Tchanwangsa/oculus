//! A model pack: the files one local model needs, each at a pinned URL, size
//! and sha256. A download streams each missing file to `<file>.part`, checks
//! its size and checksum, and only then renames it into place, so a file on
//! disk at its pinned size is a verified one. One download per pack at a
//! time; [`cancel`] or the caller's `stop` ends it with [`CANCELLED`].

mod claim;
mod fetch;
#[cfg(test)]
mod tests;

use std::path::Path;
use std::sync::atomic::Ordering;

pub use crate::runtime::stream::CANCELLED;
use claim::Claim;
pub use claim::{cancel, downloading};

pub struct PackFile {
    /// The name it is saved under in the pack's directory.
    pub file: &'static str,
    /// Pinned to a commit, so the bytes behind it never change.
    pub url: &'static str,
    pub bytes: u64,
    /// Lowercase hex.
    pub sha256: &'static str,
}

pub struct Pack {
    /// Dedupes downloads; also the cancel handle.
    pub id: &'static str,
    pub files: &'static [PackFile],
}

impl Pack {
    pub fn bytes(&self) -> u64 {
        self.files.iter().map(|f| f.bytes).sum()
    }
}

/// Where a pack is, for a status line.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    Missing,
    Downloading,
    Present,
}

/// Whether `pack` is in `dir`, or being downloaded. Local only.
pub fn status(dir: &Path, pack: &Pack) -> Status {
    if is_present(dir, pack) {
        Status::Present
    } else if downloading(pack.id) {
        Status::Downloading
    } else {
        Status::Missing
    }
}

/// Every file of `pack` is in `dir` at its pinned size. Reads metadata only:
/// the checksum was checked before the file got its name.
pub fn is_present(dir: &Path, pack: &Pack) -> bool {
    pack.files.iter().all(|f| on_disk(dir, f))
}

fn on_disk(dir: &Path, file: &PackFile) -> bool {
    std::fs::metadata(dir.join(file.file)).is_ok_and(|m| m.is_file() && m.len() == file.bytes)
}

/// Download the files of `pack` missing from `dir`, reporting `(received,
/// total)` bytes over the whole pack. Files already there count as received.
pub fn download(
    dir: &Path,
    pack: &Pack,
    progress: impl Fn(u64, u64),
    stop: impl Fn() -> bool,
) -> Result<(), String> {
    let claim = Claim::take(pack.id)?;
    let cancelled = || stop() || claim.flag().load(Ordering::Relaxed);
    std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let total = pack.bytes();
    let mut done = 0;
    progress(0, total);
    for file in pack.files {
        if !on_disk(dir, file) {
            fetch::fetch(
                file,
                &dir.join(file.file),
                &|received| progress(done + received, total),
                &cancelled,
            )?;
        }
        done += file.bytes;
    }
    progress(total, total);
    Ok(())
}

/// Delete the pack's files and any partial download, stopping the download
/// first; returns the bytes freed.
pub fn delete(dir: &Path, pack: &Pack) -> Result<u64, String> {
    cancel(pack.id);
    let mut freed = 0;
    for file in pack.files {
        for name in [file.file.to_string(), format!("{}.part", file.file)] {
            let path = dir.join(name);
            if let Ok(meta) = std::fs::metadata(&path) {
                std::fs::remove_file(&path).map_err(|e| format!("{}: {e}", path.display()))?;
                freed += meta.len();
            }
        }
    }
    Ok(freed)
}
