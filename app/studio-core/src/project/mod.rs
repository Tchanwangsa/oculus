//! A video project folder: `video.json` (size, theme, scenes, voice),
//! `components/<Name>.tsx`, `assets/`, and for a narrated video `script.md`
//! and `narration/manifest.json`. Loading parses and validates `video.json`,
//! the theme, the script and the manifest; components load when first
//! rendered. Narration is checked against the script when scene lengths are
//! needed (`scenes`), so a stale manifest fails renders but not `lint`.

mod components;
mod fingerprint;
mod frames;
mod lint;
mod narration;
mod scenes;
mod script;
mod timeline;
mod video;

pub(crate) use components::valid_name;
pub use components::{CascadeComponents, ComponentSource, FolderComponents};
pub use lint::{LintIssue, LintReport};
pub use narration::{
    LOW_CONFIDENCE, MANIFEST_PATH, Manifest, ManifestGap, ManifestScene, ManifestWord, WordCues,
};
pub use script::{Script, ScriptSection};
pub use timeline::Timeline;
pub(crate) use video::valid_subject;
pub use video::{FRAME_RATES, SceneSpec, Video, Voice};

pub(crate) use scenes::Staged;

use crate::error::{Result, StudioError};
use crate::morph::MorphColor;
use crate::render::{MotionBlur, Purpose};
use crate::theme::Theme;
use crate::transition::{BlurMode, Transition, TransitionError, blur_mode, morph_color, resolve};
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// What a load looks at beyond the video's folder. `global` is the global
/// library (`studio/library/`) and `courses` the folder holding each subject's
/// `<code>/studio/` library; a subject's library is read only when both it
/// and `courses` are given. `subject` and `theme`, when set, replace
/// video.json's for this load (the CLI's `--subject` and `--theme`). Themes
/// and components resolve through the subject's library, then the global
/// one, then (for themes) the bundled set.
#[derive(Clone, Debug, Default)]
pub struct Library {
    pub global: Option<PathBuf>,
    pub courses: Option<PathBuf>,
    pub subject: Option<String>,
    pub theme: Option<String>,
}

#[derive(Clone)]
pub struct Project {
    pub dir: PathBuf,
    pub video: Video,
    pub theme: Theme,
    /// The cascade over the project's `components/`, the subject's library
    /// and the global one, in that order.
    pub components: Arc<CascadeComponents>,
    /// `script.md`, when the video is narrated.
    pub script: Option<Script>,
    /// `narration/manifest.json`, when it exists.
    pub manifest: Option<Manifest>,
}

impl Project {
    /// The video alone: its own `components/` and the bundled themes.
    pub fn load(dir: impl AsRef<Path>) -> Result<Project> {
        Project::load_with(dir, &Library::default())
    }

    /// [`Project::load`] over `library`'s subject and global libraries.
    pub fn load_with(dir: impl AsRef<Path>, library: &Library) -> Result<Project> {
        let mut project = Project::load_unnarrated_with(dir, library)?;
        project.manifest = Manifest::read(&project.dir)?;
        Ok(project)
    }

    /// [`Project::load`] without reading `narration/manifest.json`
    /// (`manifest` is `None`): for the narrator, which rewrites a manifest
    /// this build cannot read instead of failing on it.
    pub fn load_unnarrated(dir: impl AsRef<Path>) -> Result<Project> {
        Project::load_unnarrated_with(dir, &Library::default())
    }

    pub fn load_unnarrated_with(dir: impl AsRef<Path>, library: &Library) -> Result<Project> {
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
        let subject = library.subject.clone().or_else(|| video.subject.clone());
        if let Some(s) = &subject
            && !valid_subject(s)
        {
            return Err(StudioError::Project {
                path: path.clone(),
                message: format!("subject `{s}` must be a course code's folder name"),
            });
        }
        let subject_dir = match (&subject, &library.courses) {
            (Some(s), Some(courses)) => Some(courses.join(s).join("studio")),
            _ => None,
        };
        let name = library.theme.as_deref().unwrap_or(&video.theme);
        let theme = resolve_theme(&dir, name, subject_dir.as_deref(), library)?;
        let mut sources: Vec<(String, Arc<dyn ComponentSource>)> = vec![(
            "project".to_owned(),
            Arc::new(FolderComponents::new(dir.join("components"))),
        )];
        if let (Some(s), Some(sub)) = (&subject, &subject_dir) {
            sources.push((
                format!("subject {s}"),
                Arc::new(FolderComponents::new(sub.join("components"))),
            ));
        }
        if let Some(global) = &library.global {
            sources.push((
                "global".to_owned(),
                Arc::new(FolderComponents::new(global.join("components"))),
            ));
        }
        Ok(Project {
            dir,
            video,
            theme,
            components: Arc::new(CascadeComponents::new(sources)),
            script,
            manifest: None,
        })
    }

    pub fn assets_dir(&self) -> PathBuf {
        self.dir.join("assets")
    }

    /// The video's `morphColor` (video.json `motion`, then the theme), as
    /// `<Morph>` uses it.
    pub(crate) fn morph_color(&self) -> Result<MorphColor, TransitionError> {
        morph_color(None, &self.video.motion, &self.theme)
    }

    /// The motion blur a render for `purpose` gets from the `motionBlur`
    /// token (video.json `motion`, then the theme, then `finals`), for
    /// `RenderOptions::motion_blur`, with the shutter at the video's fps.
    /// The export asks for `Purpose::Final`; `oculus studio frame` and
    /// `strip` blur only when asked.
    pub fn motion_blur(&self, purpose: Purpose) -> Result<Option<MotionBlur>> {
        let on = match blur_mode(&self.video.motion, &self.theme)? {
            BlurMode::Always => true,
            BlurMode::Finals => purpose == Purpose::Final,
            BlurMode::Off => false,
        };
        Ok(on.then(|| MotionBlur::at(self.fps())))
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

/// A theme name or a `.json` path. A path is relative to the video; a name
/// is looked up as `themes/<name>.json` in the subject's library, then the
/// global one, then the bundled set.
fn resolve_theme(
    dir: &Path,
    name: &str,
    subject_dir: Option<&Path>,
    library: &Library,
) -> Result<Theme> {
    if name.ends_with(".json") || name.contains('/') {
        return Theme::from_file(&dir.join(name));
    }
    for root in [subject_dir, library.global.as_deref()]
        .into_iter()
        .flatten()
    {
        let file = root.join("themes").join(format!("{name}.json"));
        if file.try_exists().map_err(|e| StudioError::io(&file, e))? {
            return Theme::from_file(&file);
        }
    }
    Theme::bundled(name)
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
