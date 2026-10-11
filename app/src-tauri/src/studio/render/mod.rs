//! `render`: a Studio video folder to an mp4. Each scene is its own
//! segment, encoded by its own ffmpeg from frames rendered in parallel and
//! written in order, and cached under its content address, so a re-render
//! only renders the scenes an edit touched. The segments are joined by a
//! stream copy beside the narration track, mixed here. Rules:
//! docs/studio.md, "Rendering a video".

mod audio;
mod cache;
mod encode;
mod encoder;
mod error;
mod ffmpeg;
mod key;
mod mux;
mod plan;
mod progress;
mod quality;
mod reorder;
#[cfg(test)]
mod tests;
mod workers;

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

use studio_core::{Project, RenderOptions, StudioError};

pub use error::RenderError;
use error::Stop;
use plan::Scene;
pub use progress::{Outcome, Progress, Report, SegmentReport};
pub use quality::Quality;
use quality::Settings;

#[derive(Debug, Clone)]
pub struct Options {
    pub quality: Quality,
    /// Render threads (default: half the cores), capped by memory.
    pub threads: Option<usize>,
    /// The mp4 (default: `<dir>/renders/<quality>.mp4`).
    pub out: Option<PathBuf>,
    /// The ffmpeg to run (`sources::echo360::find_ffmpeg`).
    pub ffmpeg: PathBuf,
}

/// How a render ended that was not a failure.
#[derive(Debug)]
pub enum Finished {
    Complete(Report),
    /// `cancel` was set: ffmpeg was killed and nothing partial was kept.
    Cancelled,
}

/// Render the video in `dir`. `cancel` is checked before every frame;
/// `progress` hears each step. No network: stale narration fails with
/// studio-core's "run `oculus studio narrate`" error.
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

/// The default output path for `quality`.
pub fn default_out(dir: &Path, quality: Quality) -> PathBuf {
    dir.join("renders").join(format!("{}.mp4", quality.name()))
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
    let project = Project::load(dir)?;
    let fps = project.fps();
    let size = project.video.size;
    let settings = Settings::new(
        options.quality,
        (size.width, size.height),
        fps,
        project.motion_blur(options.quality.purpose())?,
    )?;
    let scenes = scenes(&project)?;
    let trees = key::Trees::of(dir)?;
    let segments = plan::segments(dir, &scenes, &trees, &settings);
    let jobs = plan::jobs(&segments, fps);
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
        &project,
        render_options,
        &segments,
        &jobs,
        threads,
        &settings,
        options,
        cancel,
        progress,
    )?;

    let frames = segments.last().map_or(0, |s| s.frames.end);
    let clips = plan::clips(&scenes, fps, project.theme.narration.lead);
    let track = audio::track(dir, &clips, audio::length(frames, fps))?;
    if cancel.load(Ordering::Relaxed) {
        return Err(Stop::Cancelled);
    }

    progress(Progress::Muxing);
    let list = cache::concat_list(dir, options.quality);
    let paths: Vec<PathBuf> = segments.iter().map(|s| s.path.clone()).collect();
    std::fs::write(&list, mux::concat_list(&paths)).map_err(|e| RenderError::io(&list, e))?;
    let out = options
        .out
        .clone()
        .unwrap_or_else(|| default_out(dir, options.quality));
    if let Some(parent) = out.parent().filter(|p| !p.as_os_str().is_empty()) {
        std::fs::create_dir_all(parent).map_err(|e| RenderError::io(parent, e))?;
    }
    mux::mux(&options.ffmpeg, &list, track.as_deref(), &out, cancel)?;

    let used = cache::Used {
        segments: segments.iter().map(|s| format!("{}.mp4", s.key)).collect(),
        track: track
            .as_ref()
            .and_then(|t| t.file_name())
            .map(|n| n.to_string_lossy().into_owned()),
    };
    let pruned = cache::record_and_prune(dir, options.quality, used)?;
    Ok(Report {
        path: out,
        quality: options.quality,
        width: settings.width,
        height: settings.height,
        fps,
        frames,
        duration: frames as f64 / f64::from(fps),
        audio: track.is_some(),
        threads,
        segments: reports,
        pruned,
        ms: started.elapsed().as_millis() as u64,
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
