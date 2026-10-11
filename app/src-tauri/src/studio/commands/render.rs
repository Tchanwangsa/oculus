//! Rendering a video from the app: `studio_render` runs `render::render` off
//! the async runtime and reports each step as a `studio-render-progress`
//! event; `studio_render_cancel` stops it. One render per folder
//! (`render::RENDERS`). Rules: docs/studio.md, "Rendering a video".

use std::path::Path;
use std::time::{Duration, Instant};

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};

use crate::studio::render::{self, Finished, Progress, Quality, Report, RENDERS};

/// Display only, never persisted.
pub const PROGRESS_EVENT: &str = "studio-render-progress";

/// The least time between two `rendering` events; a segment's first and
/// last frame are always sent.
const EVERY: Duration = Duration::from_millis(100);

#[derive(Serialize, Clone)]
struct Event<'a> {
    /// The folder as `studio_render` was given it.
    dir: &'a str,
    #[serde(flatten)]
    step: Step<'a>,
}

#[derive(Serialize, Clone)]
#[serde(tag = "phase", rename_all = "camelCase")]
enum Step<'a> {
    /// Sent when the render starts, then again with `plan` once it is made.
    Planning {
        #[serde(skip_serializing_if = "Option::is_none")]
        plan: Option<Plan>,
    },
    /// Segment `segment` of `of` (1-based), showing scene `scene`: `frame`
    /// of its `frames` encoded, at `fps` frames a second.
    Rendering {
        segment: usize,
        of: usize,
        scene: usize,
        frame: u64,
        frames: u64,
        fps: f64,
    },
    Muxing,
    Complete {
        path: &'a Path,
        report: &'a Report,
    },
    Cancelled,
    Error {
        error: &'a str,
    },
}

#[derive(Serialize, Clone, Copy)]
#[serde(rename_all = "camelCase")]
struct Plan {
    segments: usize,
    cached: usize,
    /// Frames to render: the segments not cached.
    frames: u64,
    threads: usize,
}

/// Render the video in `dir` at `quality` to `<dir>/renders/<quality>.mp4`.
/// Resolves to the report, or `null` when cancelled; rejects with the
/// reason a render failed, or when `dir` already has a render running.
#[tauri::command]
pub async fn studio_render(
    app: AppHandle,
    dir: String,
    quality: Quality,
) -> Result<Option<Report>, String> {
    let running = RENDERS.start(Path::new(&dir), quality)?;
    crate::runtime::blocking::run(move || {
        emit(&app, &dir, Step::Planning { plan: None });
        let result = render_once(&app, &dir, quality, &running);
        // Freed before the terminal event, so a listener can start the next
        // render as soon as it hears this one end.
        drop(running);
        let name = quality.name();
        match &result {
            Ok(Some(report)) => {
                elog!(
                    "[oculus] studio {name} render of {dir} wrote {} in {:.1} s",
                    report.path.display(),
                    report.ms as f64 / 1000.0
                );
                let path = &report.path;
                emit(&app, &dir, Step::Complete { path, report });
            }
            Ok(None) => {
                elog!("[oculus] studio {name} render of {dir} cancelled");
                emit(&app, &dir, Step::Cancelled);
            }
            Err(error) => {
                elog!("[oculus] studio {name} render of {dir} failed: {error}");
                emit(&app, &dir, Step::Error { error });
            }
        }
        result
    })
    .await
}

/// Ask the render of `dir` to stop; false when none runs.
#[tauri::command]
pub fn studio_render_cancel(dir: String) -> bool {
    RENDERS.cancel(Path::new(&dir))
}

fn emit(app: &AppHandle, dir: &str, step: Step<'_>) {
    app.emit(PROGRESS_EVENT, Event { dir, step }).ok();
}

fn render_once(
    app: &AppHandle,
    dir: &str,
    quality: Quality,
    running: &render::Running,
) -> Result<Option<Report>, String> {
    let ffmpeg = crate::sources::echo360::find_ffmpeg(app.path().resource_dir().ok())
        .ok_or("ffmpeg not found; run `bun run ffmpeg` in app/, or install it")?;
    let options = render::Options {
        quality,
        threads: None,
        out: None,
        ffmpeg,
        library: crate::studio::libraries::libraries(false).map_err(|e| e.to_string())?,
    };
    let (mut of, mut scene) = (0, 0);
    let mut last: Option<Instant> = None;
    let finished = render::render(
        Path::new(dir),
        &options,
        running.cancel_flag(),
        &mut |p| match p {
            Progress::Planned {
                segments,
                cached,
                frames,
                threads,
            } => {
                let plan = Plan {
                    segments,
                    cached,
                    frames,
                    threads,
                };
                emit(app, dir, Step::Planning { plan: Some(plan) });
            }
            Progress::Segment {
                of: segments,
                scene: index,
                ..
            } => {
                (of, scene) = (segments, index);
                last = None;
            }
            Progress::Frame {
                n,
                frame,
                frames,
                fps,
            } => {
                if frame == frames || last.is_none_or(|t| t.elapsed() >= EVERY) {
                    last = Some(Instant::now());
                    let step = Step::Rendering {
                        segment: n,
                        of,
                        scene,
                        frame,
                        frames,
                        fps,
                    };
                    emit(app, dir, step);
                }
            }
            Progress::Done(_) => {}
            Progress::Muxing => emit(app, dir, Step::Muxing),
        },
    )
    .map_err(|e| e.to_string())?;
    Ok(match finished {
        Finished::Complete(report) => Some(report),
        Finished::Cancelled => None,
    })
}
