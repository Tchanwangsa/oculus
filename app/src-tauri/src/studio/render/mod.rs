//! `render`: a Studio video folder to one export preset's file. A video
//! preset renders each scene as its own segment, encoded by its own ffmpeg
//! from frames rendered in parallel and written in order, and cached under
//! its content address, so a re-render only renders the scenes an edit
//! touched; the segments are joined by a stream copy beside the narration
//! track, mixed here, and the captions are written beside it as WebVTT.
//! `audio` and `subtitles` export the track and the captions alone (`export`).
//! `RENDERS` keeps the app to one render per folder. Rules: docs/studio.md,
//! "Rendering a video".

mod audio;
mod cache;
mod encode;
mod encoder;
mod error;
mod export;
mod ffmpeg;
mod key;
mod mux;
mod plan;
mod preset;
mod progress;
mod reorder;
mod running;
mod settings;
mod status;
#[cfg(test)]
mod tests;
mod toolkit;
mod workers;

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

use studio_core::{Project, RenderOptions, StudioError};

pub use error::RenderError;
use error::Stop;
pub use key::still_key;
use plan::Scene;
pub use preset::{Output, Preset};
pub use progress::{Outcome, Progress, Report, SegmentReport};
pub use running::{Renders, Running, RENDERS};
use settings::Settings;
pub use status::{exports, ExportStatus};
pub use toolkit::Toolkit;

#[derive(Debug, Clone)]
pub struct Options {
    pub preset: Preset,
    /// Frame-rendering threads (default: half the cores), capped by memory.
    /// The encoder's own threads are fixed (`Toolkit::codec`).
    pub threads: Option<usize>,
    /// The file to write (default: `Preset::default_out`).
    pub out: Option<PathBuf>,
    /// The ffmpeg to run (`sources::echo360::find_ffmpeg`).
    pub ffmpeg: PathBuf,
    /// The libraries the video's components and themes resolve through.
    pub library: studio_core::Library,
}

/// How a render ended that was not a failure.
#[derive(Debug)]
pub enum Finished {
    Complete(Report),
    /// `cancel` was set: ffmpeg was killed and nothing partial was kept.
    Cancelled,
}

/// Render the video in `dir` to `options.preset`. `cancel` is checked
/// before every frame; `progress` hears each step. No network: stale
/// narration fails with studio-core's "run `oculus studio narrate`" error.
pub fn render(
    dir: &Path,
    options: &Options,
    cancel: &AtomicBool,
    progress: &mut dyn FnMut(Progress),
) -> Result<Finished, RenderError> {
    match run(dir, options, cancel, progress) {
        Ok(report) => Ok(Finished::Complete(report)),
        Err(Stop::Cancelled) => Ok(Finished::Cancelled),
        Err(Stop::Failed(e)) => Err(e),
    }
}

fn run(
    dir: &Path,
    options: &Options,
    cancel: &AtomicBool,
    progress: &mut dyn FnMut(Progress),
) -> Result<Report, Stop> {
    let started = Instant::now();
    // The concat list names segments by path, which ffmpeg resolves against
    // the list's folder, so every path is made absolute here.
    let dir = &std::path::absolute(dir).map_err(|e| RenderError::io(dir, e))?;
    let project = Project::load_with(dir, &options.library)?;
    let out = options
        .out
        .clone()
        .unwrap_or_else(|| options.preset.default_out(dir));
    if let Some(parent) = out.parent().filter(|p| !p.as_os_str().is_empty()) {
        std::fs::create_dir_all(parent).map_err(|e| RenderError::io(parent, e))?;
    }
    let mut report = match options.preset.output() {
        Output::Video(_) => video(dir, &project, &out, options, cancel, progress)?,
        Output::Audio => export::audio(dir, &project, &out, options, cancel, progress)?,
        Output::Subtitles => export::subtitles(dir, &project, &out, options.preset)?,
    };
    report.ms = started.elapsed().as_millis() as u64;
    Ok(report)
}

/// A video preset's plan: its settings, segments, frames to render and
/// narration clips.
struct VideoPlan {
    settings: Settings,
    segments: Vec<plan::Segment>,
    jobs: Vec<workers::Job>,
    clips: Vec<audio::Clip>,
    frames: u64,
}

/// What a render of `preset` would make of `project` now, checking first
/// that the ffmpeg `toolkit` can make it.
fn plan_video(
    dir: &Path,
    project: &Project,
    preset: Preset,
    toolkit: &Toolkit,
) -> Result<VideoPlan, RenderError> {
    let Output::Video(video) = preset.output() else {
        return Err(RenderError::Render(format!(
            "{} is not a video preset",
            preset.name()
        )));
    };
    toolkit.check_frames()?;
    let fps = project.fps();
    let size = project.video.size;
    let settings = Settings::new(
        preset,
        video,
        (size.width, size.height),
        fps,
        project.motion_blur(preset.purpose())?,
        toolkit,
    )?;
    let scenes = scenes(project)?;
    let trees = key::Trees::of(dir, project)?;
    let segments = plan::segments(dir, &scenes, &trees, &settings);
    let jobs = plan::jobs(&segments, fps);
    let clips = plan::clips(&scenes, fps, project.theme.narration.lead);
    if !clips.is_empty() {
        toolkit.check_audio()?;
    }
    let frames = segments.last().map_or(0, |s| s.frames.end);
    Ok(VideoPlan {
        settings,
        segments,
        jobs,
        clips,
        frames,
    })
}

fn video(
    dir: &Path,
    project: &Project,
    out: &Path,
    options: &Options,
    cancel: &AtomicBool,
    progress: &mut dyn FnMut(Progress),
) -> Result<Report, Stop> {
    let toolkit = Toolkit::of(&options.ffmpeg)?;
    let VideoPlan {
        settings,
        segments,
        jobs,
        clips,
        frames,
    } = plan_video(dir, project, options.preset, &toolkit)?;
    let fps = project.fps();
    let frame_bytes = u64::from(settings.width) * u64::from(settings.height) * 4;
    let threads = workers::threads(
        options.threads,
        frame_bytes,
        settings.motion_blur.is_some(),
        &project.assets_dir(),
        jobs.len(),
    );
    progress(Progress::Planned {
        segments: segments.len(),
        cached: segments.iter().filter(|s| s.cached).count(),
        frames: jobs.len() as u64,
        threads,
    });

    let segments_dir = cache::segments(dir);
    std::fs::create_dir_all(&segments_dir).map_err(|e| RenderError::io(&segments_dir, e))?;
    let render_options = RenderOptions {
        scale: settings.scale,
        motion_blur: settings.motion_blur,
    };
    let reports = encode::encode(
        project,
        render_options,
        &segments,
        &jobs,
        threads,
        &settings,
        options,
        cancel,
        progress,
    )?;

    let track = audio::track(dir, &clips, audio::length(frames, fps))?;
    if cancel.load(Ordering::Relaxed) {
        return Err(Stop::Cancelled);
    }

    progress(Progress::Muxing);
    let list = cache::concat_list(dir, options.preset);
    let paths: Vec<PathBuf> = segments.iter().map(|s| s.path.clone()).collect();
    std::fs::write(&list, mux::concat_list(&paths)).map_err(|e| RenderError::io(&list, e))?;
    mux::mux(&options.ffmpeg, &list, track.as_deref(), out, cancel)?;
    let sidecars = export::captions_beside(project, out)?.into_iter().collect();

    let used = cache::Used {
        segments: segments.iter().map(|s| format!("{}.mp4", s.key)).collect(),
        track: track.as_deref().and_then(cache::file_name),
        out: Some(out.to_string_lossy().into_owned()),
        cues: None,
    };
    let pruned = cache::record_and_prune(dir, options.preset, used)?;
    Ok(Report {
        path: out.to_owned(),
        preset: options.preset,
        width: Some(settings.width),
        height: Some(settings.height),
        fps,
        frames,
        duration: frames as f64 / f64::from(fps),
        audio: track.is_some(),
        threads,
        segments: reports,
        sidecars,
        pruned,
        ms: 0,
    })
}

/// Each scene's frames, key inputs and clip.
fn scenes(project: &Project) -> Result<Vec<Scene>, StudioError> {
    let timeline = project.timeline()?;
    (0..project.video.scenes.len())
        .map(|i| {
            Ok(Scene {
                index: i,
                id: project.video.scenes[i].id.clone(),
                frames: timeline.scene_frames(i)?,
                fingerprint: project.scene_fingerprint(i)?,
                clip: project
                    .narration(i)?
                    .map(|c| project.dir.join("narration").join(&c.audio)),
            })
        })
        .collect()
}
