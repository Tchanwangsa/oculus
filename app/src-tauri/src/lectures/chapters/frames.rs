//! Frames for a later stage: one image per boundary, and the dock's live grab.

use super::{FRAME_BYTES, FRAME_H, FRAME_W, GRAB_OFFSETS, GRAB_TOLERANCE, GRAB_WIDTH};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

/// One probed JPEG per boundary at `<out_dir>/<second>.jpg` (see
/// [`grab_frame`]); the name keeps the boundary second, not the probed offset.
/// `on_grab` gets the running count.
///
/// JPEGs a previous run left that this one will not rewrite are deleted first,
/// so the folder never holds frames from another candidate set or stream.
/// The `live/` subfolder belongs to another job and is left alone.
pub fn extract_frames(
    ffmpeg: &Path,
    video: &Path,
    secs: &[u32],
    out_dir: &Path,
    mut on_grab: impl FnMut(usize),
) -> Result<Vec<PathBuf>, String> {
    std::fs::create_dir_all(out_dir).map_err(|e| format!("{}: {e}", out_dir.display()))?;
    sweep_orphans(out_dir, secs);
    let mut written = Vec::with_capacity(secs.len());
    for &second in secs {
        let out = out_dir.join(format!("{second}.jpg"));
        grab_frame(ffmpeg, video, second, GRAB_WIDTH, &out)?;
        written.push(out);
        on_grab(written.len());
    }
    Ok(written)
}

/// Delete the `<second>.jpg` files in `out_dir` not in `keep`. Best effort;
/// any other name was not written here and is left alone.
pub(super) fn sweep_orphans(out_dir: &Path, keep: &[u32]) {
    let Ok(entries) = std::fs::read_dir(out_dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("jpg") {
            continue;
        }
        if !entry.file_type().is_ok_and(|t| t.is_file()) {
            continue;
        }
        let second = path
            .file_stem()
            .and_then(|s| s.to_str())
            .and_then(|s| s.parse::<u32>().ok());
        match second {
            Some(second) if keep.contains(&second) => continue,
            Some(_) => {
                std::fs::remove_file(&path).ok();
            }
            None => continue,
        }
    }
}

/// One JPEG of `second`, written to `out`, at most `width` px wide (never
/// upscaled).
///
/// The boundary second itself is often black (screen share restarting) or a
/// couple of seconds later the room's "connect your laptop" splash, so
/// [`GRAB_OFFSETS`] are probed with the same seek the grab uses and the
/// earliest within [`GRAB_TOLERANCE`] of the most detailed wins. Also used by
/// the dock's live grab (`app::lecture_grab_frames`).
pub fn grab_frame(
    ffmpeg: &Path,
    video: &Path,
    second: u32,
    width: u32,
    out: &Path,
) -> Result<(), String> {
    let at = best_offset(ffmpeg, video, second);
    // Escaped: an unescaped comma would end the filter.
    let scale = format!("scale=min({width}\\,iw):-2");
    let status = Command::new(ffmpeg)
        .args([
            "-v",
            "error",
            "-nostdin",
            "-y",
            "-ss",
            &at.to_string(),
            "-i",
        ])
        .arg(video)
        .args(["-frames:v", "1", "-vf", &scale, "-q:v", "3"])
        .arg(out)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map_err(|e| format!("could not run ffmpeg: {e}"))?;
    if !status.success() || !out.exists() {
        return Err(format!("no frame at {at}s"));
    }
    Ok(())
}

/// Which stream and second a lecture's thumbnail is grabbed from: source 1
/// when it is downloaded, else source 2, a quarter of the way in — past the
/// title card and the room's splash. `None` with nothing downloaded.
pub fn thumbnail_pick(
    streams: &[(crate::sources::echo360::SourceNum, PathBuf)],
    duration: u32,
) -> Option<(PathBuf, u32)> {
    let (_, video) = streams.iter().min_by_key(|(n, _)| *n)?;
    Some((video.clone(), duration / 4))
}

/// Which second to actually grab `boundary`'s frame from. Earliest-good rather
/// than best, so a sparse title slide is not passed over for a busier one.
fn best_offset(ffmpeg: &Path, video: &Path, boundary: u32) -> u32 {
    let probed: Vec<(u32, f32)> = GRAB_OFFSETS
        .iter()
        .filter_map(|off| {
            let at = boundary + off;
            frame_detail(ffmpeg, video, at).map(|detail| (at, detail))
        })
        .collect();
    pick_offset(&probed).unwrap_or(boundary)
}

/// The choosing half of [`best_offset`]: the first probe (in preference order)
/// within [`GRAB_TOLERANCE`] of the most detailed one.
pub(super) fn pick_offset(probed: &[(u32, f32)]) -> Option<u32> {
    let best = probed
        .iter()
        .map(|(_, detail)| *detail)
        .fold(f32::NEG_INFINITY, f32::max);
    probed
        .iter()
        .find(|(_, detail)| *detail >= best * GRAB_TOLERANCE)
        .map(|(at, _)| *at)
}

/// How much is going on in the frame at `second`: the standard deviation of
/// its grey values at 160×90. A blank scores ~0. `None` means no frame
/// (normally past the end).
fn frame_detail(ffmpeg: &Path, video: &Path, second: u32) -> Option<f32> {
    let out = Command::new(ffmpeg)
        .args(["-v", "error", "-nostdin", "-ss", &second.to_string(), "-i"])
        .arg(video)
        .args([
            "-frames:v",
            "1",
            "-vf",
            &format!("scale={FRAME_W}:{FRAME_H},format=gray"),
            "-f",
            "rawvideo",
            "-",
        ])
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .ok()?;
    if !out.status.success() || out.stdout.len() < FRAME_BYTES {
        return None;
    }
    Some(spread(&out.stdout[..FRAME_BYTES]))
}

/// Population standard deviation of a frame's grey values.
pub(super) fn spread(frame: &[u8]) -> f32 {
    let n = frame.len() as f32;
    let mean = frame.iter().map(|b| f32::from(*b)).sum::<f32>() / n;
    let variance = frame
        .iter()
        .map(|b| {
            let d = f32::from(*b) - mean;
            d * d
        })
        .sum::<f32>()
        / n;
    variance.sqrt()
}
