//! The output file: the segments joined by ffmpeg's concat demuxer as a
//! stream copy (every segment starts on a keyframe and shares one encoder
//! setting), with the narration track encoded to AAC beside them, moov atom
//! first (`+faststart`) so a player can start before the end has loaded.

use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;

use super::encoder::part_path;
use super::error::{RenderError, Stop};
use super::ffmpeg;

/// The concat demuxer's list: one `file '<path>'` line per segment, a `'`
/// inside a path closed, escaped and reopened.
pub(super) fn concat_list(segments: &[PathBuf]) -> String {
    segments
        .iter()
        .map(|p| {
            let path = p.to_string_lossy().replace('\'', r"'\''");
            format!("file '{path}'\n")
        })
        .collect()
}

/// Join the segments listed in `list` with `audio` (if any) into `out`,
/// through `<out>.part`.
pub(super) fn mux(
    ffmpeg: &Path,
    list: &Path,
    audio: Option<&Path>,
    out: &Path,
    cancel: &AtomicBool,
) -> Result<(), Stop> {
    let part = part_path(out);
    let mut args: Vec<&OsStr> = ["-f", "concat", "-safe", "0", "-i"]
        .iter()
        .map(OsStr::new)
        .collect();
    args.push(list.as_os_str());
    if let Some(audio) = audio {
        args.push(OsStr::new("-i"));
        args.push(audio.as_os_str());
    }
    args.extend(["-map", "0:v", "-c:v", "copy"].map(OsStr::new));
    if audio.is_some() {
        args.extend(["-map", "1:a", "-c:a", "aac", "-b:a", "160k"].map(OsStr::new));
    }
    args.extend(["-movflags", "+faststart", "-f", "mp4"].map(OsStr::new));
    args.push(part.as_os_str());
    let result = ffmpeg::run(ffmpeg, &args, "joining the segments", cancel);
    if let Err(e) = result {
        std::fs::remove_file(&part).ok();
        return Err(e);
    }
    std::fs::rename(&part, out).map_err(|e| RenderError::io(out, e))?;
    Ok(())
}
