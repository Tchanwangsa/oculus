//! `Renderer`: a project's frames by time. It loads each component on first
//! use and plays scene transitions; with `motion_blur` set it averages
//! sub-frames over the shutter (`blur`).

use super::blur::{self, MotionBlur};
use super::engine::{Engine, SceneCall};
use super::{Frame, RenderOptions, Time};
use crate::error::{Result, StudioError};
use crate::layout::LNode;
use crate::project::{ComponentSource, Project, Staged, Timeline};
use crate::transition::{Transition, TransitionError};
use std::sync::Arc;

pub struct Renderer {
    pub(super) engine: Engine,
    scenes: Vec<Staged>,
    transitions: Vec<Result<Option<Transition>, TransitionError>>,
    /// The video's `morphColor` for `<Morph>`, or the bad value that stops
    /// every frame.
    morph_color: Result<(), TransitionError>,
    timeline: Timeline,
    components: Arc<dyn ComponentSource>,
    motion_blur: Option<MotionBlur>,
}

impl Renderer {
    /// Fails when a scene's length is unknown: narration missing or made
    /// for other text (`StudioError::Narration`), or a silent scene with no
    /// hold.
    pub fn new(project: &Project, options: RenderOptions) -> Result<Renderer> {
        if let Some(mb) = &options.motion_blur {
            mb.check()?;
        }
        let scenes = (0..project.video.scenes.len())
            .map(|i| project.staged(i))
            .collect::<Result<Vec<_>>>()?;
        let timeline = Timeline::new(scenes.iter().map(|s| s.length));
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
            scenes,
            transitions: project.transitions(),
            timeline,
            components: Arc::clone(&project.components),
            motion_blur: options.motion_blur,
        })
    }

    /// The engine underneath, e.g. to inspect a loaded component's schema.
    pub fn engine(&mut self) -> &mut Engine {
        &mut self.engine
    }

    /// The video's scene lengths, narration included.
    pub fn timeline(&self) -> &Timeline {
        &self.timeline
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

    /// Scene `index`'s length in seconds.
    pub(super) fn length(&self, index: usize) -> f64 {
        self.scenes[index].length
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
            let name = s.spec.component.as_str();
            if self.engine.component(name).is_none() {
                let tsx = self.components.read(name)?;
                self.engine.load_component(name, &tsx)?;
            }
        }
        Ok(())
    }

    /// The frame at scene-relative `t`, without motion blur. `ready` first.
    pub(super) fn sharp(&mut self, index: usize, t: f64) -> Result<Frame> {
        let (a, b, into) = calls(&self.scenes, &self.transitions, index, t);
        scene_frame(&mut self.engine, a, b, into)
    }

    /// What `sharp` would paint, laid out but not painted. Hand the trees
    /// back with `Engine::discard`.
    pub(super) fn layers(&mut self, index: usize, t: f64) -> Result<Vec<LNode>> {
        let (a, b, into) = calls(&self.scenes, &self.transitions, index, t);
        let (layers, _logs) = scene_layers(&mut self.engine, a, b, into)?;
        Ok(layers)
    }
}

/// Scene `index` at `t`, the scene before it and the transition into it.
fn calls<'a>(
    scenes: &'a [Staged],
    transitions: &'a [Result<Option<Transition>, TransitionError>],
    index: usize,
    t: f64,
) -> (Option<SceneCall<'a>>, SceneCall<'a>, Option<&'a Transition>) {
    let into = transitions[index].as_ref().ok().and_then(Option::as_ref);
    let before = index.checked_sub(1).map(|i| scenes[i].call(i, 0.0));
    (before, scenes[index].call(index, t), into)
}

/// Scene call `b` through `into` (its resolved transition) while it plays,
/// with `a` (the scene before, at any `t`) held at its last moment. Both
/// components must be loaded.
pub(crate) fn scene_frame(
    engine: &mut Engine,
    a: Option<SceneCall<'_>>,
    b: SceneCall<'_>,
    into: Option<&Transition>,
) -> Result<Frame> {
    let (layers, logs) = scene_layers(engine, a, b, into)?;
    engine.compose(layers, logs, b.component)
}

/// The laid-out trees `scene_frame` paints, in order, and the logs.
fn scene_layers(
    engine: &mut Engine,
    a: Option<SceneCall<'_>>,
    b: SceneCall<'_>,
    into: Option<&Transition>,
) -> Result<(Vec<LNode>, Vec<String>)> {
    match (into, a) {
        (Some(tr), Some(a)) if tr.covers(b.t) => {
            let a = SceneCall {
                t: last_moment(a.duration),
                ..a
            };
            engine.transition_layers(a, b, tr, b.t)
        }
        _ => {
            let p = engine.prepare(b)?;
            Ok((vec![p.tree], p.logs))
        }
    }
}

/// The last moment of a scene of `length` seconds: Sequence and Stagger
/// windows are half-open, so at exactly `length` their children have ended
/// and would vanish.
pub(crate) fn last_moment(length: f64) -> f64 {
    (length - 1e-6).max(length / 2.0)
}
