//! A scene laid out every `STEP` seconds and at each cue word, alone (no
//! transition), flattened into items (`items`). Layouts run on worker
//! threads, one `Engine` each, and come back in time order, so the result
//! is the same at any thread count.

use super::items::{Item, items};
use super::limits::STEP;
use crate::error::{Result, StudioError};
use crate::project::{ComponentSource, CuedWord, Project, Staged};
use crate::render::{Engine, RenderOptions};
use std::collections::HashSet;

/// One scene's samples and what the checks need to read them.
pub struct SceneLook {
    pub index: usize,
    pub id: Option<String>,
    pub component: String,
    pub length: f64,
    /// The transition into it plays over `[0, head)`.
    pub head: f64,
    /// The frame's width and height in px.
    pub size: (f64, f64),
    /// Its words at the times `word()` returns; empty when silent.
    pub words: Vec<CuedWord>,
    /// While the narration plays, scene seconds.
    pub narration: Option<(f64, f64)>,
    pub(crate) samples: Vec<Sample>,
}

pub(crate) struct Sample {
    pub t: f64,
    pub items: Vec<Item>,
}

impl SceneLook {
    /// The scene as `scene 5 `mod``.
    pub fn name(&self) -> String {
        match &self.id {
            Some(id) => format!("scene {} `{id}`", self.index),
            None => format!("scene {}", self.index),
        }
    }
}

/// One sample's items, or why it could not be laid out.
type Laid = Result<Vec<Item>>;
/// A sample by its scene's place in the request and its own index.
type Job = (usize, usize);

/// What a worker needs for one scene.
struct Plan {
    index: usize,
    staged: Staged,
    times: Vec<f64>,
}

impl Project {
    /// Scenes `scenes` sampled, one result each, in order. A scene fails
    /// with its length's error (stale narration) or the earliest layout
    /// that failed.
    pub fn look(&self, scenes: &[usize]) -> Vec<Result<SceneLook>> {
        let plans: Vec<Result<Plan>> = scenes.iter().map(|&i| self.plan(i)).collect();
        let jobs: Vec<Job> = plans
            .iter()
            .enumerate()
            .filter_map(|(p, plan)| plan.as_ref().ok().map(|plan| (p, plan.times.len())))
            .flat_map(|(p, n)| (0..n).map(move |k| (p, k)))
            .collect();
        let cores = std::thread::available_parallelism().map_or(1, usize::from);
        let threads = (cores / 2).clamp(1, 8).min(jobs.len().max(1));
        let ready: Vec<Option<&Plan>> = plans.iter().map(|p| p.as_ref().ok()).collect();
        let mut laid: Vec<Vec<Option<Laid>>> = plans
            .iter()
            .map(|p| p.as_ref().map_or(0, |p| p.times.len()))
            .map(|n| (0..n).map(|_| None).collect())
            .collect();
        let results: Vec<Vec<(Job, Laid)>> = std::thread::scope(|s| {
            let workers: Vec<_> = (0..threads)
                .map(|k| {
                    let (jobs, ready) = (&jobs, &ready);
                    s.spawn(move || {
                        let mine = jobs.iter().skip(k).step_by(threads).copied();
                        work(self, ready, mine)
                    })
                })
                .collect();
            workers
                .into_iter()
                .map(|w| {
                    w.join().unwrap_or_else(|_| {
                        let e = StudioError::Render("a look worker thread panicked".into());
                        vec![(jobs[0], Err(e))]
                    })
                })
                .collect()
        });
        for ((p, k), r) in results.into_iter().flatten() {
            laid[p][k] = Some(r);
        }
        let timeline = self.timeline().ok();
        plans
            .into_iter()
            .zip(laid)
            .map(|(plan, laid)| {
                let plan = plan?;
                let mut samples = Vec::with_capacity(plan.times.len());
                for (&t, r) in plan.times.iter().zip(laid) {
                    let r = r.unwrap_or_else(|| {
                        Err(StudioError::Render("a look sample was not laid out".into()))
                    });
                    let items = r.map_err(|e| match &timeline {
                        Some(tl) => StudioError::Frame {
                            t: tl.start(plan.index) + t,
                            scene: plan.index,
                            scene_t: t,
                            source: Box::new(e),
                        },
                        None => e,
                    })?;
                    samples.push(Sample { t, items });
                }
                self.scene_look(plan, samples)
            })
            .collect()
    }

    fn plan(&self, index: usize) -> Result<Plan> {
        let staged = self.staged(index)?;
        let words = self.cued_words(index)?.unwrap_or_default();
        Ok(Plan {
            index,
            times: times(staged.length, &words),
            staged,
        })
    }

    fn scene_look(&self, plan: Plan, samples: Vec<Sample>) -> Result<SceneLook> {
        let index = plan.index;
        let head = match self.transitions().into_iter().nth(index) {
            Some(Ok(Some(tr))) if index > 0 && tr.covers(0.0) => tr.dur.min(plan.staged.length),
            _ => 0.0,
        };
        let lead = self.theme.narration.lead;
        let narration = self
            .narration(index)?
            .map(|clip| (lead, lead + clip.duration));
        let size = self.video.size;
        Ok(SceneLook {
            index,
            id: plan.staged.spec.id.clone(),
            component: plan.staged.spec.component.clone(),
            length: plan.staged.length,
            head,
            size: (f64::from(size.width), f64::from(size.height)),
            words: self.cued_words(index)?.unwrap_or_default(),
            narration,
            samples,
        })
    }
}

/// Every `STEP` from 0, the scene's end as lint renders it (1 ms before
/// its length; the held moment a transition shows is lint's to check),
/// and each word's start.
pub(crate) fn times(length: f64, words: &[CuedWord]) -> Vec<f64> {
    let end = length - (length / 2.0).min(1e-3);
    let mut out: Vec<f64> = (0..)
        .map(|i| f64::from(i) * STEP)
        .take_while(|&t| t < end)
        .collect();
    out.push(end);
    out.extend(
        words
            .iter()
            .map(|w| w.start)
            .filter(|&t| t >= 0.0 && t < end),
    );
    out.sort_by(f64::total_cmp);
    out.dedup_by(|a, b| (*a - *b).abs() < 1e-6);
    out
}

/// One worker's samples: a fresh engine, each component loaded on first
/// use, each tree flattened and handed back.
fn work(
    project: &Project,
    plans: &[Option<&Plan>],
    jobs: impl Iterator<Item = Job>,
) -> Vec<(Job, Laid)> {
    let options = RenderOptions {
        scale: 0.25,
        motion_blur: None,
    };
    let engine = Engine::new(
        project.theme.clone(),
        project.video.size,
        Some(project.assets_dir()),
        options,
    );
    let mut engine = match engine {
        Ok(mut e) => {
            if let Ok(c) = project.morph_color() {
                e.set_morph_color(c);
            }
            e
        }
        Err(e) => {
            let msg = e.to_string();
            return jobs
                .map(|j| (j, Err(StudioError::Render(msg.clone()))))
                .collect();
        }
    };
    let mut loaded: HashSet<String> = HashSet::new();
    let mut out = Vec::new();
    for (p, k) in jobs {
        let Some(plan) = plans[p] else { continue };
        let name = &plan.staged.spec.component;
        if !loaded.contains(name) {
            let load = project
                .components
                .read(name)
                .and_then(|tsx| engine.load_component(name, &tsx).map(|_| ()));
            if let Err(e) = load {
                out.push(((p, k), Err(e)));
                continue;
            }
            loaded.insert(name.clone());
        }
        let t = plan.times[k];
        let r = engine.prepare(plan.staged.call(plan.index, t)).map(|p| {
            let items = items(std::slice::from_ref(&p.tree));
            engine.release(vec![p.tree]);
            items
        });
        out.push(((p, k), r));
    }
    out
}
