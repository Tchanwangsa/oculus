//! One pack file: HTTP to `<file>.part`, verified, then renamed into place.

use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use sha2::{Digest, Sha256};

use super::{PackFile, CANCELLED};

/// No bytes for this long is a stalled connection, not a slow one; the whole
/// download has no deadline.
const STALL: Duration = Duration::from_secs(60);

pub(super) fn fetch(
    file: &PackFile,
    dest: &Path,
    progress: &dyn Fn(u64),
    cancelled: &dyn Fn() -> bool,
) -> Result<(), String> {
    let agent = ureq::AgentBuilder::new().timeout_read(STALL).build();
    let response = agent.get(file.url).call().map_err(|e| match e {
        ureq::Error::Status(status, _) => format!("the server answered {status} for {}", file.url),
        e => format!("could not reach {}: {e}", file.url),
    })?;
    let length = response
        .header("content-length")
        .and_then(|s| s.parse::<u64>().ok());
    if let Some(length) = length.filter(|&l| l != file.bytes) {
        return Err(format!(
            "{} is {length} bytes on the server, not the pinned {}",
            file.file, file.bytes
        ));
    }
    write_verified(&mut response.into_reader(), dest, file, progress, cancelled)
}

/// Stream `reader` to `<dest>.part`, hashing as it goes, and rename it to
/// `dest` only when its size and sha256 match `file`'s. The part is removed
/// on any failure, so a retry starts clean.
pub(super) fn write_verified(
    reader: &mut dyn Read,
    dest: &Path,
    file: &PackFile,
    progress: &dyn Fn(u64),
    cancelled: &dyn Fn() -> bool,
) -> Result<(), String> {
    let part = part_path(dest);
    let result = (|| {
        let mut out =
            std::fs::File::create(&part).map_err(|e| format!("{}: {e}", part.display()))?;
        let mut hash = Sha256::new();
        let mut buf = vec![0u8; 1 << 16];
        let mut received = 0u64;
        let mut last = Instant::now();
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
            received += n as u64;
            if received > file.bytes {
                return Err(format!(
                    "{} ran past its pinned {} bytes",
                    file.file, file.bytes
                ));
            }
            hash.update(&buf[..n]);
            out.write_all(&buf[..n])
                .map_err(|e| format!("{}: {e}", part.display()))?;
            if last.elapsed() >= Duration::from_millis(150) {
                last = Instant::now();
                progress(received);
            }
        }
        if received != file.bytes {
            return Err(format!(
                "the download of {} stopped at {received} of {} bytes",
                file.file, file.bytes
            ));
        }
        let digest = format!("{:x}", hash.finalize());
        if digest != file.sha256 {
            return Err(format!(
                "{} failed its checksum (sha256 {digest}, expected {}); it was deleted",
                file.file, file.sha256
            ));
        }
        out.sync_all()
            .map_err(|e| format!("{}: {e}", part.display()))?;
        drop(out);
        std::fs::rename(&part, dest).map_err(|e| format!("{}: {e}", dest.display()))?;
        progress(received);
        Ok(())
    })();
    if result.is_err() {
        std::fs::remove_file(&part).ok();
    }
    result
}

fn part_path(dest: &Path) -> PathBuf {
    let mut part = dest.as_os_str().to_os_string();
    part.push(".part");
    PathBuf::from(part)
}
