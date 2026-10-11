//! A video project folder: `video.json` (size, theme, scenes),
//! `components/<Name>.tsx` and `assets/`. Loading parses and validates
//! `video.json` and the theme; components load when first rendered.

mod components;
mod lint;
mod timeline;

pub use components::{ComponentSource, FolderComponents};
pub use lint::{LintIssue, LintReport};
pub use timeline::Timeline;

use crate::error::{Result, StudioError};
use crate::morph::MorphColor;
use crate::render::Size;
use crate::theme::Theme;
use crate::transition::{
    MotionSpec, Transition, TransitionError, TransitionSpec, morph_color, resolve,
};
use serde::Deserialize;
use serde_json::{Map, Value};
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// `video.json`.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Video {
    pub size: Size,
    /// A bundled theme name, or a `.json` path relative to the project.
    pub theme: String,
    pub scenes: Vec<SceneSpec>,
    /// Transition defaults for this video, over the theme's.
    #[serde(default)]
    pub motion: MotionSpec,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SceneSpec {
    pub component: String,
    #[serde(default)]
    pub props: Map<String, Value>,
    /// Seconds the scene lasts.
    pub hold: f64,
    /// How this scene comes in, over the video's and theme's defaults.
    #[serde(default)]
    pub transition: Option<TransitionSpec>,
}

#[derive(Clone)]
pub struct Project {
    pub dir: PathBuf,
    pub video: Video,
    pub theme: Theme,
    pub components: Arc<dyn ComponentSource>,
}

impl Project {
    pub fn load(dir: impl AsRef<Path>) -> Result<Project> {
        let dir = dir.as_ref().to_path_buf();
        let path = dir.join("video.json");
        let fail = |message: String| StudioError::Project {
            path: path.clone(),
            message,
        };
        let json = std::fs::read_to_string(&path).map_err(|e| StudioError::io(&path, e))?;
        let video: Video = serde_json::from_str(&json).map_err(|e| fail(e.to_string()))?;
        if video.size.width == 0
            || video.size.height == 0
            || video.size.width > 8192
            || video.size.height > 8192
        {
            return Err(fail(format!(
                "size {}x{} must be 1 to 8192 px a side",
                video.size.width, video.size.height
            )));
        }
        if video.scenes.is_empty() {
            return Err(fail(
                "scenes is empty; a video needs at least one scene".to_owned(),
            ));
        }
        for (i, s) in video.scenes.iter().enumerate() {
            if !(s.hold.is_finite() && s.hold > 0.0) {
                return Err(fail(format!(
                    "scenes[{i}].hold is {}; it must be a positive number of seconds",
                    s.hold
                )));
            }
        }
        let theme = if video.theme.ends_with(".json") || video.theme.contains('/') {
            Theme::from_file(&dir.join(&video.theme))?
        } else {
            Theme::bundled(&video.theme)?
        };
        let components = Arc::new(FolderComponents::new(dir.join("components")));
        Ok(Project {
            dir,
            video,
            theme,
            components,
        })
    }

    pub fn assets_dir(&self) -> PathBuf {
        self.dir.join("assets")
    }

    pub fn timeline(&self) -> Timeline {
        Timeline::new(self.video.scenes.iter().map(|s| s.hold))
    }

    /// Total length in seconds.
    pub fn length(&self) -> f64 {
        self.timeline().length()
    }

    /// The transition into each scene through the cascade (`None` for the
    /// first scene), or the bad value that stops it.
    /// The video's `morphColor` (video.json `motion`, then the theme), as
    /// `<Morph>` uses it.
    pub(crate) fn morph_color(&self) -> Result<MorphColor, TransitionError> {
        morph_color(None, &self.video.motion, &self.theme)
    }

    pub(crate) fn transitions(&self) -> Vec<Result<Option<Transition>, TransitionError>> {
        let scenes = self.video.scenes.iter().enumerate();
        scenes
            .map(|(i, s)| resolve(i, s.transition.as_ref(), &self.video.motion, &self.theme))
            .collect()
    }
}
