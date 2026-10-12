//! Locating ffmpeg and trimming the fixed lead-in off a download.

use std::path::{Path, PathBuf};

fn is_runnable(path: &Path) -> bool {
    crate::runtime::platform::command(path)
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

/// Seconds of intro the university puts on every recording (the institution
/// profile's `echo360_lead_in_secs`); 0 for most.
pub fn lead_in_secs(data_dir: &Path) -> u32 {
    crate::sources::institution::load(data_dir)
        .map(|i| i.echo360_lead_in_secs)
        .unwrap_or(0)
}

/// Drop `lead_in` seconds with a stream copy (no re-encode); with none, the
/// download is the file.
pub fn trim_video(ffmpeg: &Path, raw: &Path, out: &Path, lead_in: u32) -> bool {
    if lead_in == 0 {
        return std::fs::rename(raw, out).is_ok();
    }
    crate::runtime::platform::command(ffmpeg)
        .args([
            "-y",
            "-ss",
            &lead_in.to_string(),
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
