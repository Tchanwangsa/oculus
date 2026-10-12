//! Downloading an embed: both fMP4s stream into a scratch directory, ffmpeg
//! joins them without re-encoding, and only then do the video and its
//! captions move into the library. The scratch directory goes on every exit.

use std::cell::Cell;
use std::path::{Path, PathBuf};

use super::cdn;
use super::files::MediaFiles;
use crate::runtime::stream::{copy_to_file, percent};

/// Share of the bar the video takes when the two sizes aren't both known up
/// front; the audio is the smaller stream.
const VIDEO_SHARE_UNKNOWN: f64 = 0.8;

pub struct Job<'a> {
    pub files: &'a MediaFiles,
    /// Signed.
    pub captions_url: Option<&'a str>,
    pub ffmpeg: &'a Path,
    /// Emptied first, removed at the end; on the same volume as `dest`.
    pub scratch: &'a Path,
    pub dest: &'a Path,
}

/// Removes the scratch directory when the download ends, however it ends.
struct ScratchDir(PathBuf);

impl Drop for ScratchDir {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.0).ok();
    }
}

/// Run `job`; returns the video's size. `on_progress` gets whole percents of
/// the bytes fetched, `on_muxing` fires once both are in, and `cancelled` is
/// polled per chunk (ending with [`crate::runtime::stream::CANCELLED`]).
pub fn download(
    job: &Job,
    on_progress: &dyn Fn(u8),
    on_muxing: &dyn Fn(),
    cancelled: &dyn Fn() -> bool,
) -> Result<u64, String> {
    std::fs::remove_dir_all(job.scratch).ok();
    std::fs::create_dir_all(job.scratch)
        .map_err(|e| format!("create {}: {e}", job.scratch.display()))?;
    let scratch = ScratchDir(job.scratch.to_path_buf());

    let share = video_share(
        cdn::head_length(&job.files.video),
        cdn::head_length(&job.files.audio),
    );
    let last = Cell::new(u8::MAX);
    let report = |pct: u8| {
        if pct != last.replace(pct) {
            on_progress(pct);
        }
    };

    let video = scratch.0.join("video.mp4.part");
    let audio = scratch.0.join("audio.mp4.part");
    for (url, path, what, from, span) in [
        (&job.files.video, &video, "the video stream", 0.0, share),
        (
            &job.files.audio,
            &audio,
            "the audio stream",
            share,
            1.0 - share,
        ),
    ] {
        if cancelled() {
            return Err(crate::runtime::stream::CANCELLED.to_string());
        }
        let (mut body, total) = cdn::open(url, what)?;
        copy_to_file(
            &mut body,
            path,
            total,
            &|done| {
                if let Some(total) = total.filter(|&t| t > 0) {
                    let part = f64::from(percent(done, total)) / 100.0;
                    report(((from + span * part) * 100.0).round().min(100.0) as u8);
                }
            },
            cancelled,
        )?;
    }

    on_muxing();
    let muxed = scratch.0.join("muxed.mp4");
    mux(job.ffmpeg, &video, &audio, &muxed)?;
    let captions = match job.captions_url {
        Some(url) => {
            let vtt = scratch.0.join("captions.vtt");
            let bytes = cdn::get_bytes(url, "the captions")?;
            std::fs::write(&vtt, bytes).map_err(|e| format!("write captions: {e}"))?;
            Some(vtt)
        }
        None => None,
    };

    if let Some(parent) = job.dest.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    std::fs::rename(&muxed, job.dest)
        .map_err(|e| format!("move into {}: {e}", job.dest.display()))?;
    if let Some(vtt) = captions {
        let beside = captions_path(job.dest);
        std::fs::rename(&vtt, &beside)
            .map_err(|e| format!("move into {}: {e}", beside.display()))?;
    }
    std::fs::metadata(job.dest)
        .map(|m| m.len())
        .map_err(|e| e.to_string())
}

/// `<video>.vtt`, the sibling the media player reads captions from.
pub fn captions_path(video: &Path) -> PathBuf {
    let mut name = video.as_os_str().to_os_string();
    name.push(".vtt");
    PathBuf::from(name)
}

/// The video's share of the bar: its share of the bytes when both sizes are
/// known, else a fixed split.
pub(super) fn video_share(video: Option<u64>, audio: Option<u64>) -> f64 {
    match (video, audio) {
        (Some(v), Some(a)) if v + a > 0 => v as f64 / (v + a) as f64,
        _ => VIDEO_SHARE_UNKNOWN,
    }
}

/// Join one video and one audio stream into an MP4, copying both.
fn mux(ffmpeg: &Path, video: &Path, audio: &Path, out: &Path) -> Result<(), String> {
    let output = std::process::Command::new(ffmpeg)
        .args(["-y", "-hide_banner", "-loglevel", "error", "-i"])
        .arg(video)
        .arg("-i")
        .arg(audio)
        .args([
            "-map",
            "0:v",
            "-map",
            "1:a",
            "-c",
            "copy",
            "-movflags",
            "+faststart",
        ])
        .arg(out)
        .output()
        .map_err(|e| format!("ffmpeg could not start: {e}"))?;
    if output.status.success() {
        return Ok(());
    }
    let stderr = String::from_utf8_lossy(&output.stderr);
    Err(format!(
        "ffmpeg could not join the video and audio: {}",
        stderr.trim()
    ))
}
