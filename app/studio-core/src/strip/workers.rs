//! Frames in parallel: one `Renderer` per thread (never vello's own
//! threads, which panic with filters), frames dealt round-robin, results
//! put back by index so the sheet is the same at any thread count.

use super::StripOptions;
use crate::error::{Result, StudioError};
use crate::project::Project;
use crate::render::{Frame, RenderOptions, Renderer, Time};
use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};

/// What all renderers together may hold.
const BUDGET: u64 = 2 << 30;
/// One renderer before its frame buffers: the QuickJS runtime (heap capped
/// at 256 MB), parley's fonts and text pool, vello's context. A 1080p
/// renderer measured about 11 MB with its buffers; 64 MB covers heavy ones.
const BASE: u64 = 64 << 20;

/// Threads for `n` frames: the option, else half the cores, never more
/// than `n`, and only as many as fit the memory budget.
pub fn threads(project: &Project, o: &StripOptions, n: usize) -> usize {
    let cores = std::thread::available_parallelism().map_or(1, usize::from);
    let wanted = o.threads.unwrap_or(cores / 2).clamp(1, n.max(1));
    let fit = (BUDGET / per_renderer(project, o)).max(1);
    wanted.min(usize::try_from(fit).unwrap_or(usize::MAX))
}

/// A renderer's estimated peak: `BASE`, vello's target pixmap, the
/// frame it returns and one in flight, the motion-blur accumulator (16
/// bytes a pixel) and every decoded image in `assets/`.
fn per_renderer(project: &Project, o: &StripOptions) -> u64 {
    let side = |v: u32| (f64::from(v) * o.scale).round().max(1.0) as u64;
    let frame = side(project.video.size.width) * side(project.video.size.height) * 4;
    let blur = if o.motion_blur.is_some() {
        5 * frame
    } else {
        0
    };
    BASE + 3 * frame + blur + decoded_images(&project.assets_dir())
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
                image::image_dimensions(&path).map_or(0, |(w, h)| u64::from(w) * u64::from(h) * 4)
            }
        })
        .sum()
}

/// Every frame, in order, or the error of the earliest frame that failed.
pub fn render_all(
    project: &Project,
    options: RenderOptions,
    times: &[f64],
    threads: usize,
) -> Result<Vec<Frame>> {
    // The earliest failed index; workers skip frames after it, so the
    // error reported does not depend on scheduling.
    let first_bad = AtomicUsize::new(usize::MAX);
    let results: Vec<(usize, Result<Frame>)> = std::thread::scope(|s| {
        let workers: Vec<_> = (0..threads)
            .map(|k| {
                let first_bad = &first_bad;
                s.spawn(move || work(project, options, times, (k, threads), first_bad))
            })
            .collect();
        workers
            .into_iter()
            .enumerate()
            .flat_map(|(k, w)| {
                w.join().unwrap_or_else(|_| {
                    let e = StudioError::Render("a strip worker thread panicked".into());
                    vec![(k, Err(e))]
                })
            })
            .collect()
    });
    let mut frames: Vec<Option<Frame>> = (0..times.len()).map(|_| None).collect();
    let mut failed: Option<(usize, StudioError)> = None;
    for (i, r) in results {
        match r {
            Ok(f) => frames[i] = Some(f),
            Err(e) if failed.as_ref().is_none_or(|(j, _)| i < *j) => failed = Some((i, e)),
            Err(_) => {}
        }
    }
    if let Some((_, e)) = failed {
        return Err(e);
    }
    frames
        .into_iter()
        .collect::<Option<Vec<_>>>()
        .ok_or_else(|| StudioError::Render("strip: a frame was not rendered".into()))
}

/// Worker `k` of `step`: frames k, k + step, ...
fn work(
    project: &Project,
    options: RenderOptions,
    times: &[f64],
    (k, step): (usize, usize),
    first_bad: &AtomicUsize,
) -> Vec<(usize, Result<Frame>)> {
    let mut renderer = match Renderer::new(project, options) {
        Ok(r) => r,
        Err(e) => {
            first_bad.fetch_min(k, Ordering::Relaxed);
            return vec![(k, Err(e))];
        }
    };
    let timeline = project.timeline();
    let mut out = Vec::new();
    for i in (k..times.len()).step_by(step) {
        if i > first_bad.load(Ordering::Relaxed) {
            break;
        }
        let t = times[i];
        match renderer.frame(Time::Global(t)) {
            Ok(f) => out.push((i, Ok(f))),
            Err(e) => {
                first_bad.fetch_min(i, Ordering::Relaxed);
                let e = match timeline.locate(Time::Global(t)) {
                    Ok((scene, scene_t)) => StudioError::Frame {
                        t,
                        scene,
                        scene_t,
                        source: Box::new(e),
                    },
                    Err(_) => e,
                };
                out.push((i, Err(e)));
                break;
            }
        }
    }
    out
}
