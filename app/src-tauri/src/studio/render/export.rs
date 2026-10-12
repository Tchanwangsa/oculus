//! The presets without frames: `audio`, the narration track as AAC in an
//! m4a, and `subtitles`, the captions' phrases as WebVTT with SubRip beside
//! it; and the `.vtt` a video render writes beside its mp4.

use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;

use sha2::{Digest, Sha256};
use studio_core::{captions, Project};

use super::encoder::part_path;
use super::error::{RenderError, Stop};
use super::preset::Preset;
use super::progress::{Progress, Report};
use super::toolkit::Toolkit;
use super::{audio, cache, ffmpeg, plan, scenes, Options};

/// The narration track, encoded as the video's would be, at `out`.
pub(super) fn audio(
    dir: &Path,
    project: &Project,
    out: &Path,
    options: &Options,
    cancel: &AtomicBool,
    progress: &mut dyn FnMut(Progress),
) -> Result<Report, Stop> {
    let toolkit = Toolkit::of(&options.ffmpeg)?;
    let fps = project.fps();
    let scenes = scenes(project)?;
    let clips = plan::clips(&scenes, fps, project.theme.narration.lead);
    if clips.is_empty() {
        return Err(no_narration(dir).into());
    }
    toolkit.check_audio()?;
    let frames = scenes.last().map_or(0, |s| s.frames.end);
    let track =
        audio::track(dir, &clips, audio::length(frames, fps))?.ok_or_else(|| no_narration(dir))?;
    progress(Progress::Muxing);
    let part = part_path(out);
    let mut args: Vec<&OsStr> = vec![OsStr::new("-i"), track.as_os_str()];
    args.extend(
        [
            "-map",
            "0:a",
            "-c:a",
            "aac",
            "-b:a",
            "160k",
            "-movflags",
            "+faststart",
            "-f",
            "mp4",
        ]
        .map(OsStr::new),
    );
    args.push(part.as_os_str());
    if let Err(e) = ffmpeg::run(&options.ffmpeg, &args, "encoding the narration", cancel) {
        std::fs::remove_file(&part).ok();
        return Err(e);
    }
    std::fs::rename(&part, out).map_err(|e| RenderError::io(out, e))?;
    let used = cache::Used {
        track: cache::file_name(&track),
        out: Some(out.to_string_lossy().into_owned()),
        ..cache::Used::default()
    };
    let pruned = cache::record_and_prune(dir, options.preset, used)?;
    Ok(report(
        out,
        options.preset,
        frames,
        fps,
        true,
        Vec::new(),
        pruned,
    ))
}

/// The captions as WebVTT at `out` and SubRip beside it. No ffmpeg.
pub(super) fn subtitles(
    dir: &Path,
    project: &Project,
    out: &Path,
    preset: Preset,
) -> Result<Report, Stop> {
    let cues = project.caption_cues()?;
    if cues.is_empty() {
        return Err(no_narration(dir).into());
    }
    let vtt = captions::vtt(&cues);
    write(out, &vtt)?;
    let srt = out.with_extension("srt");
    write(&srt, &captions::srt(&cues))?;
    let used = cache::Used {
        out: Some(out.to_string_lossy().into_owned()),
        cues: Some(cues_hash(&vtt)),
        ..cache::Used::default()
    };
    let pruned = cache::record_and_prune(dir, preset, used)?;
    let frames = project.frames()?;
    Ok(report(
        out,
        preset,
        frames,
        project.fps(),
        false,
        vec![srt],
        pruned,
    ))
}

/// The captions as WebVTT beside a video render's `out` (`.vtt` for its
/// extension); `None` when the video has no narration.
pub(super) fn captions_beside(
    project: &Project,
    out: &Path,
) -> Result<Option<PathBuf>, RenderError> {
    let cues = project.caption_cues()?;
    if cues.is_empty() {
        return Ok(None);
    }
    let path = out.with_extension("vtt");
    write(&path, &captions::vtt(&cues))?;
    Ok(Some(path))
}

/// sha256 hex of a subtitle file's text.
pub(super) fn cues_hash(text: &str) -> String {
    format!("{:x}", Sha256::digest(text.as_bytes()))
}

fn write(path: &Path, text: &str) -> Result<(), RenderError> {
    let part = part_path(path);
    std::fs::write(&part, text).map_err(|e| RenderError::io(&part, e))?;
    std::fs::rename(&part, path).map_err(|e| RenderError::io(path, e))
}

fn no_narration(dir: &Path) -> RenderError {
    RenderError::Render(format!(
        "{} has no narration to export; run `oculus studio narrate` first",
        dir.display()
    ))
}

fn report(
    out: &Path,
    preset: Preset,
    frames: u64,
    fps: u32,
    audio: bool,
    sidecars: Vec<PathBuf>,
    pruned: Vec<String>,
) -> Report {
    Report {
        path: out.to_owned(),
        preset,
        width: None,
        height: None,
        fps,
        frames,
        duration: frames as f64 / f64::from(fps),
        audio,
        threads: 0,
        segments: Vec::new(),
        sidecars,
        pruned,
        ms: 0,
    }
}
