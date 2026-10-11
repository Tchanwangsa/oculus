//! A video project folder: `video.json` (size, theme, scenes, voice),
//! `components/<Name>.tsx`, `assets/`, and for a narrated video `script.md`
//! and `narration/manifest.json`. Loading parses and validates `video.json`,
//! the theme, the script and the manifest; components load when first
//! rendered. Narration is checked against the script when scene lengths are
//! needed (`scenes`), so a stale manifest fails renders but not `lint`.

mod components;
mod lint;
mod narration;
mod scenes;
mod script;
mod timeline;
mod video;

pub use components::{ComponentSource, FolderComponents};
pub use lint::{LintIssue, LintReport};
pub use narration::{
    LOW_CONFIDENCE, MANIFEST_PATH, MAX_NARRATION_SECS, Manifest, ManifestGap, ManifestScene,
    ManifestWord, WordCues,
};
pub use script::{Script, ScriptSection};
pub use timeline::Timeline;
pub use video::{SceneSpec, Video, Voice};

pub(crate) use scenes::Staged;

use crate::error::{Result, StudioError};
use crate::morph::MorphColor;
use crate::render::{MotionBlur, Purpose};
use crate::theme::Theme;
use crate::transition::{BlurMode, Transition, TransitionError, blur_mode, morph_color, resolve};
use std::path::{Path, PathBuf};
use std::sync::Arc;

#[derive(Clone)]
pub struct Project {
    pub dir: PathBuf,
    pub video: Video,
    pub theme: Theme,
    pub components: Arc<dyn ComponentSource>,
    /// `script.md`, when the video is narrated.
    pub script: Option<Script>,
    /// `narration/manifest.json`, when it exists.
    pub manifest: Option<Manifest>,
}

impl Project {
    pub fn load(dir: impl AsRef<Path>) -> Result<Project> {
        let mut project = Project::load_unnarrated(dir)?;
        project.manifest = Manifest::read(&project.dir)?;
        Ok(project)
    }

    /// [`Project::load`] without reading `narration/manifest.json`
    /// (`manifest` is `None`): for the narrator, which rewrites a manifest
    /// this build cannot read instead of failing on it.
    pub fn load_unnarrated(dir: impl AsRef<Path>) -> Result<Project> {
        let dir = dir.as_ref().to_path_buf();
        let path = dir.join("video.json");
        let json = std::fs::read_to_string(&path).map_err(|e| StudioError::io(&path, e))?;
        let script = read_script(&dir)?;
        let video: Video = serde_json::from_str(&json)
            .map_err(|e| e.to_string())
            .and_then(|v| video::check(&v, script.is_some()).map(|()| v))
            .map_err(|message| StudioError::Project {
                path: path.clone(),
                message,
            })?;
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
            script,
            manifest: None,
        })
    }

    pub fn assets_dir(&self) -> PathBuf {
        self.dir.join("assets")
    }

    /// Every scene's length in order; fails on the first scene whose length
    /// is unknown (see `scene_length`).
    pub fn timeline(&self) -> Result<Timeline> {
        let lengths = (0..self.video.scenes.len())
            .map(|i| self.scene_length(i))
            .collect::<Result<Vec<_>>>()?;
        Ok(Timeline::new(lengths.into_iter()))
    }

    /// Total length in seconds.
    pub fn length(&self) -> Result<f64> {
        Ok(self.timeline()?.length())
    }

    /// The video's `morphColor` (video.json `motion`, then the theme), as
    /// `<Morph>` uses it.
    pub(crate) fn morph_color(&self) -> Result<MorphColor, TransitionError> {
        morph_color(None, &self.video.motion, &self.theme)
    }

    /// The motion blur a render for `purpose` gets from the `motionBlur`
    /// token (video.json `motion`, then the theme, then `finals`), for
    /// `RenderOptions::motion_blur`. The export asks for `Purpose::Final`;
    /// `oculus studio frame` and `strip` blur only when asked.
    pub fn motion_blur(&self, purpose: Purpose) -> Result<Option<MotionBlur>> {
        let on = match blur_mode(&self.video.motion, &self.theme)? {
            BlurMode::Always => true,
            BlurMode::Finals => purpose == Purpose::Final,
            BlurMode::Off => false,
        };
        Ok(on.then(MotionBlur::default))
    }

    /// The transition into each scene through the cascade (`None` for the
    /// first scene), or the bad value that stops it.
    pub(crate) fn transitions(&self) -> Vec<Result<Option<Transition>, TransitionError>> {
        let scenes = self.video.scenes.iter().enumerate();
        scenes
            .map(|(i, s)| resolve(i, s.transition.as_ref(), &self.video.motion, &self.theme))
            .collect()
    }
}

/// `<dir>/script.md`, parsed, when it exists.
fn read_script(dir: &Path) -> Result<Option<Script>> {
    let path = dir.join("script.md");
    match std::fs::read_to_string(&path) {
        Ok(src) => Script::parse(&src)
            .map(Some)
            .map_err(|message| StudioError::Project { path, message }),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(StudioError::io(&path, e)),
    }
}
