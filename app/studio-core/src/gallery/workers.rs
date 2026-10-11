//! Examples in parallel: one `Engine` per thread (its JS runtime is not
//! `Send`), examples dealt round-robin, results put back by index.

use super::GalleryOptions;
use super::Moment;
use crate::error::{Result, StudioError};
use crate::render::{Engine, Frame, RenderOptions, SceneCall, Size};
use crate::theme::Theme;
use serde_json::{Map, Value};
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

/// What all engines together may hold: the strip's budget.
const BUDGET: u64 = 2 << 30;
/// One engine before its frame buffers (the QuickJS heap, text pool and
/// vello context), as the strip estimates it.
const BASE: u64 = 64 << 20;

/// Threads for `n` examples: the option, else half the cores, never more
/// than `n`, and only as many engines as fit the memory budget.
pub fn threads(size: Size, options: &GalleryOptions, n: usize) -> usize {
    let cores = std::thread::available_parallelism().map_or(1, usize::from);
    let wanted = options.threads.unwrap_or(cores / 2).clamp(1, n.max(1));
    let side = |v: u32| (f64::from(v) * options.scale).round().max(1.0) as u64;
    let frame = side(size.width) * side(size.height) * 4;
    let per_engine = BASE + 3 * frame;
    let fit = (BUDGET / per_engine).max(1);
    wanted.min(usize::try_from(fit).unwrap_or(usize::MAX))
}

pub struct Inputs<'a> {
    pub name: &'a str,
    pub tsx: &'a str,
    pub theme: &'a Theme,
    pub size: Size,
    pub assets: Option<PathBuf>,
    pub scale: f64,
    pub moment: Moment,
}

/// Every example's frame, in declaration order, or the error of the
/// earliest example that failed.
pub fn render_all(
    inputs: &Inputs<'_>,
    examples: &[(String, Map<String, Value>)],
    threads: usize,
) -> Result<Vec<Frame>> {
    let first_bad = AtomicUsize::new(usize::MAX);
    let results: Vec<(usize, Result<Frame>)> = std::thread::scope(|s| {
        let workers: Vec<_> = (0..threads)
            .map(|k| {
                let first_bad = &first_bad;
                s.spawn(move || work(inputs, examples, (k, threads), first_bad))
            })
            .collect();
        workers
            .into_iter()
            .enumerate()
            .flat_map(|(k, w)| {
                w.join().unwrap_or_else(|_| {
                    let e = StudioError::Render("a gallery worker thread panicked".into());
                    vec![(k, Err(e))]
                })
            })
            .collect()
    });
    let mut frames: Vec<Option<Frame>> = (0..examples.len()).map(|_| None).collect();
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
        .ok_or_else(|| StudioError::Render("gallery: an example was not rendered".into()))
}

/// Worker `k` of `step`: examples k, k + step, ...
fn work(
    inputs: &Inputs<'_>,
    examples: &[(String, Map<String, Value>)],
    (k, step): (usize, usize),
    first_bad: &AtomicUsize,
) -> Vec<(usize, Result<Frame>)> {
    let options = RenderOptions {
        scale: inputs.scale,
        motion_blur: None,
    };
    let engine = Engine::new(
        inputs.theme.clone(),
        inputs.size,
        inputs.assets.clone(),
        options,
    )
    .and_then(|mut e| {
        e.load_component(inputs.name, inputs.tsx)
            .map(|_| ())
            .map(|()| e)
    });
    let mut engine = match engine {
        Ok(e) => e,
        Err(e) => {
            first_bad.fetch_min(k, Ordering::Relaxed);
            return vec![(k, Err(e))];
        }
    };
    let mut out = Vec::new();
    for i in (k..examples.len()).step_by(step) {
        if i > first_bad.load(Ordering::Relaxed) {
            break;
        }
        let (name, props) = &examples[i];
        let call = SceneCall {
            component: inputs.name,
            props,
            t: inputs.moment.t,
            duration: inputs.moment.duration,
            index: 0,
            id: None,
            words: None,
        };
        match engine.render(call) {
            Ok(frame) => out.push((i, Ok(frame))),
            Err(e) => {
                first_bad.fetch_min(i, Ordering::Relaxed);
                let e = StudioError::Render(format!("example `{name}`: {e}"));
                out.push((i, Err(e)));
                break;
            }
        }
    }
    out
}
