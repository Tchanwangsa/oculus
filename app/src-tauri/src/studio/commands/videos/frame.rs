//! A still of a video at a moment, as PNG bytes.

use std::path::Path;

use serde::Deserialize;
use studio_core::{Library, Project, RenderOptions, Renderer, Time};

use crate::studio::videos;

/// The widest still asked for; wider is a mistake, not a preview.
pub const MAX_WIDTH: u32 = 4096;

/// Where to take the still: mirrors `FrameAt` in
/// `app/src/lib/studio/videos.ts`.
#[derive(Debug, Clone, Copy, Deserialize, PartialEq)]
#[serde(untagged)]
pub enum FrameAt {
    /// `t` seconds into scene `scene` (counting from 0).
    Scene { scene: usize, t: f64 },
    /// Seconds into the whole video.
    Global { global: f64 },
}

impl From<FrameAt> for Time {
    fn from(at: FrameAt) -> Self {
        match at {
            FrameAt::Scene { scene, t } => Time::Scene { index: scene, t },
            FrameAt::Global { global } => Time::Global(global),
        }
    }
}

/// The frame of video `slug` at `at`, `width` pixels wide, with the layout
/// of the full-size frame. Stale or missing narration fails with
/// studio-core's message: a still is of the video as narrated, never a
/// guess at it.
pub fn frame_png(
    videos: &Path,
    library: &Library,
    slug: &str,
    at: FrameAt,
    width: u32,
) -> Result<Vec<u8>, String> {
    if !(1..=MAX_WIDTH).contains(&width) {
        return Err(format!("width {width} must be 1 to {MAX_WIDTH} px"));
    }
    let dir = videos::video_in(videos, slug)?;
    let project = Project::load_with(&dir, library).map_err(|e| e.to_string())?;
    let scale = f64::from(width) / f64::from(project.video.size.width);
    png(&project, at.into(), scale)
}

/// `project` at `at`, rendered at `scale`, as a PNG.
pub(super) fn png(project: &Project, at: Time, scale: f64) -> Result<Vec<u8>, String> {
    let options = RenderOptions {
        scale,
        motion_blur: None,
    };
    let mut renderer = Renderer::new(project, options).map_err(|e| e.to_string())?;
    let frame = renderer.frame(at).map_err(|e| e.to_string())?;
    frame.encode_png().map_err(|e| e.to_string())
}
