//! `Renderer`: a project's frames by time. It loads each component on first
//! use and plays scene transitions; with `motion_blur` set it averages
//! sub-frames over the shutter (`blur`).

use super::blur::{self, MotionBlur};
use super::engine::{Engine, SceneCall};
use super::{Frame, RenderOptions, Time};
use crate::error::{Result, StudioError};
use crate::layout::LNode;
use crate::project::{ComponentSource, Project, SceneSpec, Timeline};
use crate::transition::{Transition, TransitionError};
use std::sync::Arc;

pub struct Renderer {
    pub(super) engine: Engine,
    scenes: Vec<SceneSpec>,
    transitions: Vec<Result<Option<Transition>, TransitionError>>,
    /// The video's `morphColor` for `<Morph>`, or the bad value that stops
    /// every frame.
    morph_color: Result<(), TransitionError>,
    timeline: Timeline,
    components: Arc<dyn ComponentSource>,
    motion_blur: Option<MotionBlur>,
}

impl Renderer {
    pub fn new(project: &Project, options: RenderOptions) -> Result<Renderer> {
        if let Some(mb) = &options.motion_blur {
            mb.check()?;
        }
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
            motion_blur: options.motion_blur,
        })
    }

    /// The engine underneath, e.g. to inspect a loaded component's schema.
    pub fn engine(&mut self) -> &mut Engine {
        &mut self.engine
    }

    /// One frame. In the first `dur` seconds of a scene after the first, it
    /// is the transition from the scene before (see `transition`). With
    /// motion blur on, it averages sub-frames inside the scene (see `blur`).
    pub fn frame(&mut self, at: Time) -> Result<Frame> {
        let (index, t) = self.timeline.locate(at)?;
        self.ready(index)?;
        match self.motion_blur {
            Some(mb) => blur::frame(self, index, t, mb),
            None => self.sharp(index, t),
        }
    }

    /// Scene `index`'s hold in seconds.
    pub(super) fn hold(&self, index: usize) -> f64 {
        self.scenes[index].hold
    }

    /// Checks the scene's transition and loads the components a frame of it
    /// needs.
    fn ready(&mut self, index: usize) -> Result<()> {
        let count = self.scenes.len();
        let out_of_range = || StudioError::SceneOutOfRange { index, count };
        let into = self.transitions.get(index).ok_or_else(out_of_range)?;
        into.as_ref().map_err(|e| StudioError::from(e.clone()))?;
        if let Err(e) = &self.morph_color {
            return Err(e.clone().into());
        }
        let scene = self.scenes.get(index).ok_or_else(out_of_range)?;
        let before = index.checked_sub(1).and_then(|i| self.scenes.get(i));
        for s in std::iter::once(scene).chain(before) {
            let name = s.component.as_str();
            if self.engine.component(name).is_none() {
                let tsx = self.components.read(name)?;
                self.engine.load_component(name, &tsx)?;
            }
        }
        Ok(())
    }

    /// The frame at scene-relative `t`, without motion blur. `ready` first.
    pub(super) fn sharp(&mut self, index: usize, t: f64) -> Result<Frame> {
        let into = self.transitions[index]
            .as_ref()
            .ok()
            .and_then(Option::as_ref);
        scene_frame(&mut self.engine, &self.scenes, index, into, t)
    }

    /// What `sharp` would paint, laid out but not painted. Hand the trees
    /// back with `Engine::discard`.
    pub(super) fn layers(&mut self, index: usize, t: f64) -> Result<Vec<LNode>> {
        let into = self.transitions[index]
            .as_ref()
            .ok()
            .and_then(Option::as_ref);
        let (layers, _logs) = scene_layers(&mut self.engine, &self.scenes, index, into, t)?;
        Ok(layers)
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
    let (layers, logs) = scene_layers(engine, scenes, index, into, t)?;
    engine.compose(layers, logs, &scenes[index].component)
}

/// The laid-out trees `scene_frame` paints, in order, and the logs.
fn scene_layers(
    engine: &mut Engine,
    scenes: &[SceneSpec],
    index: usize,
    into: Option<&Transition>,
    t: f64,
) -> Result<(Vec<LNode>, Vec<String>)> {
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
        (Some(tr), Some(prev)) if tr.covers(t) => engine.transition_layers(
            call(prev, last_moment(scenes[prev].hold)),
            call(index, t),
            tr,
            t,
        ),
        _ => {
            let p = engine.prepare(call(index, t))?;
            Ok((vec![p.tree], p.logs))
        }
    }
}

/// The last moment of a hold: Sequence and Stagger windows are half-open,
/// so at exactly `hold` their children have ended and would vanish.
pub(crate) fn last_moment(hold: f64) -> f64 {
    (hold - 1e-6).max(hold / 2.0)
}
