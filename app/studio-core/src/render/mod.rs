//! The public rendering API: a `Renderer` turns a project and a time into a
//! `Frame`. It owns one JS runtime and one raster context, so it is not
//! shared across threads; render in parallel with one renderer per thread.

mod engine;
mod frame;

pub use engine::{Engine, SceneCall};
pub use frame::Frame;

use crate::error::Result;
use crate::project::{ComponentSource, Project, SceneSpec, Timeline};
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
    timeline: Timeline,
    components: Arc<dyn ComponentSource>,
}

impl Renderer {
    pub fn new(project: &Project, options: RenderOptions) -> Result<Renderer> {
        Ok(Renderer {
            engine: Engine::new(
                project.theme.clone(),
                project.video.size,
                Some(project.assets_dir()),
                options,
            )?,
            scenes: project.video.scenes.clone(),
            timeline: project.timeline(),
            components: Arc::clone(&project.components),
        })
    }

    /// The engine underneath, e.g. to inspect a loaded component's schema.
    pub fn engine(&mut self) -> &mut Engine {
        &mut self.engine
    }

    pub fn frame(&mut self, at: Time) -> Result<Frame> {
        let (index, t) = self.timeline.locate(at)?;
        let scene = self
            .scenes
            .get(index)
            .ok_or(crate::error::StudioError::SceneOutOfRange {
                index,
                count: self.scenes.len(),
            })?;
        let name = scene.component.as_str();
        if self.engine.component(name).is_none() {
            let tsx = self.components.read(name)?;
            self.engine.load_component(name, &tsx)?;
        }
        self.engine.render(SceneCall {
            component: name,
            props: &scene.props,
            t,
            duration: scene.hold,
            index,
        })
    }
}
