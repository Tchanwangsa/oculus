//! Making a video from the app: the slug comes from the title, and the
//! scaffold is loaded once before it is handed back.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use studio_core::Library;

use crate::studio::libraries::default_theme;
use crate::studio::videos::{self, NewVideo};

/// Mirrors `NewVideoOptions` in `app/src/lib/studio/videos.ts`, plus the
/// voice block a caller may set.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NewVideoOptions {
    pub title: Option<String>,
    pub subject: Option<String>,
    pub theme: Option<String>,
    #[serde(default)]
    pub portrait: bool,
    pub voice: Option<VoiceBlock>,
}

/// video.json's `voice` fields; each overrides Settings -> Voice for this
/// video.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VoiceBlock {
    pub engine: Option<studio_core::VoiceEngine>,
    pub model: Option<String>,
    pub voice: Option<String>,
    pub speed: Option<f64>,
}

#[derive(Debug, Serialize, PartialEq)]
pub struct Made {
    pub slug: String,
    pub dir: PathBuf,
}

/// Scaffold a video under `videos` with a unique slug from the title, in the
/// theme asked for or else its subject's default (`default_theme`). A
/// scaffold that does not load over `library` (a theme it cannot find, for
/// one) is removed again and the reason returned.
pub fn create_in(
    videos: &Path,
    library: &Library,
    options: NewVideoOptions,
) -> Result<Made, String> {
    let title = options
        .title
        .as_deref()
        .map(str::trim)
        .filter(|t| !t.is_empty());
    let slug = videos::unique_slug(videos, title);
    let voice = options.voice.map(|v| studio_core::Voice {
        engine: v.engine,
        model: v.model.filter(|s| !s.trim().is_empty()),
        voice: v.voice.filter(|s| !s.trim().is_empty()),
        speed: v.speed,
    });
    let subject = options.subject.filter(|s| !s.trim().is_empty());
    let theme = match options.theme.filter(|s| !s.trim().is_empty()) {
        Some(theme) => theme,
        None => default_theme(library, subject.as_deref())?,
    };
    let dir = videos::create_in(
        videos,
        &NewVideo {
            slug: slug.clone(),
            subject,
            title: title.map(str::to_owned),
            portrait: options.portrait,
            theme: Some(theme),
            voice,
        },
    )?;
    if let Err(e) = studio_core::Project::load_with(&dir, library) {
        std::fs::remove_dir_all(&dir).ok();
        return Err(e.to_string());
    }
    Ok(Made { slug, dir })
}
