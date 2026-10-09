//! Locating ffmpeg and trimming the fixed lead-in off a download.

use super::TRIM_SECS;

use std::path::{Path, PathBuf};

fn is_runnable(path: &Path) -> bool {
    std::process::Command::new(path)
        .arg("-version")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

/// The ffmpeg we ship, else a system install. The CLI passes `None`.
pub fn find_ffmpeg(resource_dir: Option<PathBuf>) -> Option<PathBuf> {
    // The shipped copy, or the one `bun run ffmpeg` writes in dev.
    let mut candidates = crate::runtime::bundled::candidates("ffmpeg", resource_dir);

    const SYSTEM: &[&str] = &[
        "ffmpeg",
        r"C:\ProgramData\scoop\shims\ffmpeg.exe",
        r"C:\ffmpeg\bin\ffmpeg.exe",
        r"C:\Program Files\ffmpeg\bin\ffmpeg.exe",
        "/opt/homebrew/bin/ffmpeg",
        "/usr/local/bin/ffmpeg",
        "/usr/bin/ffmpeg",
    ];
    candidates.extend(SYSTEM.iter().map(PathBuf::from));

    candidates.into_iter().find(|p| is_runnable(p))
}

/// Drop the lead-in with a stream copy (no re-encode).
pub fn trim_video(ffmpeg: &Path, raw: &Path, out: &Path) -> bool {
    std::process::Command::new(ffmpeg)
        .args([
            "-y",
            "-ss",
            &TRIM_SECS.to_string(),
            "-i",
            raw.to_str().unwrap_or(""),
            "-c",
            "copy",
            out.to_str().unwrap_or(""),
        ])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}
