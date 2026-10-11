//! The public rendering API: a `Renderer` turns a project and a time into a
//! `Frame`. It owns one JS runtime and one raster context, so it is not
//! shared across threads; render in parallel with one renderer per thread.

mod engine;
mod frame;

pub use engine::{Engine, SceneCall};
pub use frame::Frame;

use crate::error::{Result, StudioError};
use crate::project::{ComponentSource, Project, SceneSpec, Timeline};
use crate::transition::{Transition, TransitionError};
use serde::Deserialize;
use std::sync::Arc;

/// Logical frame size in px; layout always happens at this size.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Size {
    pub width: u32,
    pub height: u32,
}

#[derive(Clone, Copy, Debug)]
pub struct RenderOptions {
    /// Output scale over the same layout: 0.5 gives a 540p proxy of a 1080p
    /// video with identical layout.
    pub scale: f64,
}

impl Default for RenderOptions {
    fn default() -> Self {
        RenderOptions { scale: 1.0 }
    }
}

/// A moment in a video.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Time {
    /// Seconds from the start of the video.
    Global(f64),
    /// Seconds from the start of scene `index` (0-based).
    Scene { index: usize, t: f64 },
}

pub struct Renderer {
    engine: Engine,
    scenes: Vec<SceneSpec>,
    transitions: Vec<Result<Option<Transition>, TransitionError>>,
    /// The video's `morphColor` for `<Morph>`, or the bad value that stops
    /// every frame.
    morph_color: Result<(), TransitionError>,
    timeline: Timeline,
    components: Arc<dyn ComponentSource>,
}

impl Renderer {
    pub fn new(project: &Project, options: RenderOptions) -> Result<Renderer> {
        let mut engine = Engine::new(
            project.theme.clone(),
            project.video.size,
            Some(project.assets_dir()),
            options,
        )?;
        let morph_color = project.morph_color().map(|c| engine.set_morph_color(c));
        Ok(Renderer {
            engine,
            morph_color,
            scenes: project.video.scenes.clone(),
            transitions: project.transitions(),
            timeline: project.timeline(),
            components: Arc::clone(&project.components),
        })
    }

    /// The engine underneath, e.g. to inspect a loaded component's schema.
    pub fn engine(&mut self) -> &mut Engine {
        &mut self.engine
    }

    /// One frame. In the first `dur` seconds of a scene after the first, it
    /// is the transition from the scene before (see `transition`).
    pub fn frame(&mut self, at: Time) -> Result<Frame> {
        let (index, t) = self.timeline.locate(at)?;
        let count = self.scenes.len();
        let out_of_range = || StudioError::SceneOutOfRange { index, count };
        let scene = self.scenes.get(index).ok_or_else(out_of_range)?;
        let into = self.transitions.get(index).ok_or_else(out_of_range)?;
        let into = into.as_ref().map_err(|e| StudioError::from(e.clone()))?;
        if let Err(e) = &self.morph_color {
            return Err(e.clone().into());
        }
        let before = index.checked_sub(1).and_then(|i| self.scenes.get(i));
        for s in std::iter::once(scene).chain(before) {
            let name = s.component.as_str();
            if self.engine.component(name).is_none() {
                let tsx = self.components.read(name)?;
                self.engine.load_component(name, &tsx)?;
            }
        }
        scene_frame(&mut self.engine, &self.scenes, index, into.as_ref(), t)
    }
}

/// Scene `index` at `t`, through `into` (its resolved transition) while it
/// plays. Both scenes' components must be loaded.
pub(crate) fn scene_frame(
    engine: &mut Engine,
    scenes: &[SceneSpec],
    index: usize,
    into: Option<&Transition>,
    t: f64,
) -> Result<Frame> {
    let call = |i: usize, t: f64| {
        let s = &scenes[i];
        SceneCall {
            component: &s.component,
            props: &s.props,
            t,
            duration: s.hold,
            index: i,
        }
    };
    match (into, index.checked_sub(1)) {
        (Some(tr), Some(prev)) if tr.covers(t) => engine.render_transition(
            call(prev, last_moment(scenes[prev].hold)),
            call(index, t),
            tr,
            t,
        ),
        _ => engine.render(call(index, t)),
    }
}

/// The outgoing scene is held at the left limit of its hold: Sequence and
/// Stagger windows are half-open, so at exactly `hold` their children have
/// ended and would vanish.
fn last_moment(hold: f64) -> f64 {
    (hold - 1e-6).max(hold / 2.0)
}
