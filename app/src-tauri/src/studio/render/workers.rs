//! Frames in parallel, written in order. Each worker builds its own
//! `Renderer` inside its thread (it is not `Send`; never vello's own
//! threads, which panic with filters) and claims frames from a bounded
//! reorder buffer of `2 × threads`; the calling thread takes them in order
//! and hands each to `write`. The thread policy is `strip`'s (half the
//! cores, capped so the renderers' estimated memory stays under 2 GB),
//! which studio-core keeps private, so it is restated here.

use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};

use studio_core::{Frame, Project, RenderOptions, Renderer, StudioError, Time};

use super::error::{RenderError, Stop};
use super::reorder::Reorder;

/// What all renderers and held frames together may take.
const BUDGET: u64 = 2 << 30;
/// One renderer before its frame buffers (strip's measured estimate).
const BASE: u64 = 64 << 20;

/// One output frame: scene `scene` at scene-relative `t`, which is
/// `global` in the whole video.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct Job {
    pub scene: usize,
    pub t: f64,
    pub global: f64,
}

/// Worker threads for `jobs` frames of `frame_bytes`: `requested`, else
/// half the cores; at most one per frame, and only as many as the memory
/// budget holds.
pub(super) fn threads(
    requested: Option<usize>,
    frame_bytes: u64,
    blur: bool,
    assets: &Path,
    jobs: usize,
) -> usize {
    let cores = std::thread::available_parallelism().map_or(1, usize::from);
    let wanted = requested.unwrap_or(cores / 2).clamp(1, jobs.max(1));
    // vello's pixmap, the frame returned and one in flight, the motion-blur
    // accumulator (16 bytes a pixel), two reorder slots, decoded images.
    let blur = if blur { 5 * frame_bytes } else { 0 };
    let each = BASE + 5 * frame_bytes + blur + decoded_images(assets);
    let fit = (BUDGET / each).max(1);
    wanted.min(usize::try_from(fit).unwrap_or(usize::MAX))
}

/// RGBA bytes of every image under `dir`, from the file headers.
fn decoded_images(dir: &Path) -> u64 {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return 0;
    };
    entries
        .flatten()
        .map(|e| {
            let path = e.path();
            if path.is_dir() {
                decoded_images(&path)
            } else {
                imagesize::size(&path).map_or(0, |s| s.width as u64 * s.height as u64 * 4)
            }
        })
        .sum()
}

/// Render every job on `threads` workers and hand each frame to `write` in
/// job order. Stops at the first frame that fails (the earliest, whatever
/// the scheduling), at `write`'s first error, or on `cancel`, checked per
/// frame.
pub(super) fn render(
    project: &Project,
    options: RenderOptions,
    jobs: &[Job],
    threads: usize,
    cancel: &AtomicBool,
    mut write: impl FnMut(usize, Frame) -> Result<(), Stop>,
) -> Result<(), Stop> {
    let reorder: Reorder<Result<Frame, StudioError>> = Reorder::new(jobs.len(), 2 * threads);
    let panicked = AtomicBool::new(false);
    std::thread::scope(|s| {
        let workers: Vec<_> = (0..threads)
            .map(|_| s.spawn(|| work(project, options, jobs, &reorder, cancel, &panicked)))
            .collect();
        let result = (|| {
            for i in 0..jobs.len() {
                if cancel.load(Ordering::Relaxed) {
                    return Err(Stop::Cancelled);
                }
                match reorder.take() {
                    Some(Ok(frame)) => write(i, frame)?,
                    Some(Err(e)) => return Err(e.into()),
                    None if cancel.load(Ordering::Relaxed) => return Err(Stop::Cancelled),
                    None if panicked.load(Ordering::Relaxed) => {
                        return Err(
                            RenderError::Render("a render worker thread panicked".into()).into(),
                        )
                    }
                    None => {
                        return Err(
                            RenderError::Render(format!("frame {i} was not rendered")).into()
                        )
                    }
                }
            }
            Ok(())
        })();
        reorder.stop();
        for w in workers {
            // A panic already stopped the buffer; joining here keeps the
            // scope from re-raising it.
            w.join().ok();
        }
        result
    })
}

/// One worker: its own renderer, then frames until there are none left.
fn work(
    project: &Project,
    options: RenderOptions,
    jobs: &[Job],
    reorder: &Reorder<Result<Frame, StudioError>>,
    cancel: &AtomicBool,
    panicked: &AtomicBool,
) {
    let _guard = StopOnPanic { reorder, panicked };
    let mut renderer = match Renderer::new(project, options) {
        Ok(r) => r,
        Err(e) => {
            if let Some(i) = reorder.claim() {
                reorder.put(i, Err(e));
            }
            return;
        }
    };
    while let Some(i) = reorder.claim() {
        if cancel.load(Ordering::Relaxed) {
            reorder.stop();
            return;
        }
        let job = jobs[i];
        let frame = renderer
            .frame(Time::Scene {
                index: job.scene,
                t: job.t,
            })
            .map_err(|e| StudioError::Frame {
                t: job.global,
                scene: job.scene,
                scene_t: job.t,
                source: Box::new(e),
            });
        reorder.put(i, frame);
    }
}

/// Stops the buffer when its worker unwinds, so the reader is not left
/// waiting for a frame that will never come.
struct StopOnPanic<'a> {
    reorder: &'a Reorder<Result<Frame, StudioError>>,
    panicked: &'a AtomicBool,
}

impl Drop for StopOnPanic<'_> {
    fn drop(&mut self) {
        if std::thread::panicking() {
            self.panicked.store(true, Ordering::Relaxed);
            self.reorder.stop();
        }
    }
}
