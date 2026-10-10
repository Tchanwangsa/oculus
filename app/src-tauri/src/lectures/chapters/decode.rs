//! The decode pass: ffmpeg frames to the diff between neighbours.

use super::{FRAME_BYTES, FRAME_H, FRAME_W};
use std::io::Read;
use std::path::Path;
use std::process::{Command, Stdio};

/// Mean absolute difference between each sampled frame (`fps=1`, so frame *n*
/// is second *n*; the first entry is second 1) and the one before it. Frames
/// are diffed as they arrive rather than held.
///
/// `on_frame` fires per frame; the caller throttles.
pub fn sample_diffs(
    ffmpeg: &Path,
    video: &Path,
    mut on_frame: impl FnMut(u32),
) -> Result<Vec<(u32, f32)>, String> {
    let mut previous = vec![0u8; FRAME_BYTES];
    let mut diffs: Vec<(u32, f32)> = Vec::new();
    each_frame(ffmpeg, video, "error", &[], |index, frame| {
        if index > 0 {
            diffs.push((index, mean_abs_diff(&previous, frame)));
            on_frame(index);
        }
        previous.copy_from_slice(frame);
    })?;
    if diffs.is_empty() {
        return Err("no video frames decoded".to_string());
    }
    Ok(diffs)
}

/// The mean brightness (0–255) of each second of the last `secs` seconds of
/// `video`, from one decode of the tail alone (`-sseof`), and the file's own
/// length from ffmpeg's log. ffmpeg ignores a seek before the start, so a
/// shorter file is read whole.
pub fn tail_luma(
    ffmpeg: &Path,
    video: &Path,
    secs: u32,
) -> Result<(Vec<f32>, Option<f64>), String> {
    let seek = format!("-{secs}");
    let mut luma: Vec<f32> = Vec::new();
    let log = each_frame(ffmpeg, video, "info", &["-sseof", &seek], |_, frame| {
        let total: u64 = frame.iter().map(|&b| u64::from(b)).sum();
        luma.push(total as f32 / frame.len() as f32);
    })?;
    if luma.is_empty() {
        return Err("no video frames decoded".to_string());
    }
    Ok((luma, crate::transcribe::audio::duration(&log)))
}

/// One `fps=1`, 160×90 greyscale decode of `video` to a pipe, each frame
/// handed to `on_frame` with its index as it arrives; answers ffmpeg's log at
/// `level`. `seek` goes before `-i`.
fn each_frame(
    ffmpeg: &Path,
    video: &Path,
    level: &str,
    seek: &[&str],
    mut on_frame: impl FnMut(u32, &[u8]),
) -> Result<String, String> {
    let mut child = Command::new(ffmpeg)
        .args(["-v", level, "-nostdin", "-hide_banner", "-nostats"])
        .args(seek)
        .arg("-i")
        .arg(video)
        .args([
            "-vf",
            &format!("fps=1,scale={FRAME_W}:{FRAME_H},format=gray"),
            "-f",
            "rawvideo",
            "-",
        ])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("could not run ffmpeg: {e}"))?;

    // Drained on its own thread: ffmpeg blocks on a full stderr pipe.
    let mut stderr = child.stderr.take().expect("piped stderr");
    let errors = std::thread::spawn(move || {
        let mut text = String::new();
        stderr.read_to_string(&mut text).ok();
        text
    });

    let mut stdout = child.stdout.take().expect("piped stdout");
    let mut current = vec![0u8; FRAME_BYTES];
    let mut index: u32 = 0;

    loop {
        match read_frame(&mut stdout, &mut current) {
            Ok(true) => {}
            Ok(false) => break,
            Err(e) => {
                child.kill().ok();
                child.wait().ok();
                return Err(format!("reading frames: {e}"));
            }
        }
        on_frame(index, &current);
        index += 1;
    }

    let status = child.wait().map_err(|e| e.to_string())?;
    let text = errors.join().unwrap_or_default();
    if !status.success() {
        let detail = text
            .lines()
            .last()
            .unwrap_or("no detail")
            .trim()
            .to_string();
        return Err(format!("ffmpeg failed: {detail}"));
    }
    Ok(text)
}

/// Fill `frame` completely, or report that the stream ended. A trailing
/// partial frame (ffmpeg cut off mid-write) is dropped.
pub(super) fn read_frame(source: &mut impl Read, frame: &mut [u8]) -> std::io::Result<bool> {
    let mut filled = 0;
    while filled < frame.len() {
        match source.read(&mut frame[filled..])? {
            0 => return Ok(false),
            n => filled += n,
        }
    }
    Ok(true)
}

pub(super) fn mean_abs_diff(a: &[u8], b: &[u8]) -> f32 {
    let total: u64 = a
        .iter()
        .zip(b.iter())
        .map(|(x, y)| u64::from(x.abs_diff(*y)))
        .sum();
    total as f32 / a.len() as f32
}
