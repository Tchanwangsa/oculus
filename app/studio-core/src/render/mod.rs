//! The public rendering API: a `Renderer` turns a project and a time into a
//! `Frame`. It owns one JS runtime and one raster context, so it is not
//! shared across threads; render in parallel with one renderer per thread.

mod blur;
mod engine;
mod frame;
mod renderer;

pub use blur::{MotionBlur, Purpose};
pub use engine::{Engine, SceneCall};
pub use frame::Frame;
pub use renderer::Renderer;

pub(crate) use frame::write_png;
pub(crate) use renderer::{last_moment, scene_frame};

use serde::Deserialize;

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
    /// Temporal supersampling, off by default. `Renderer` applies it;
    /// `Engine` renders one call and ignores it.
    pub motion_blur: Option<MotionBlur>,
}

impl Default for RenderOptions {
    fn default() -> Self {
        RenderOptions {
            scale: 1.0,
            motion_blur: None,
        }
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
