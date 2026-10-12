//! One video for its page: the row, its format and theme, whether it burns
//! captions in, each scene with its place in time and its words, and which
//! exports exist.

use std::collections::HashMap;
use std::path::Path;
use std::time::UNIX_EPOCH;

use serde::Serialize;
use studio_core::{Library, Project};

use super::rows::VideoView;
use crate::db::store::StudioLink;
use crate::studio::render::Preset;
use crate::studio::videos;

/// Mirrors `VideoDetail` in `app/src/lib/studio/videos.ts`.
#[derive(Debug, Serialize, PartialEq)]
pub struct VideoDetail {
    pub row: VideoView,
    pub fps: u32,
    pub width: u32,
    pub height: u32,
    pub theme: String,
    pub voice: Option<String>,
    /// video.json's `captions`: the renders have captions burned in.
    pub captions: bool,
    pub scenes: Vec<SceneInfo>,
    pub renders: Vec<RenderFile>,
}

#[derive(Debug, Serialize, PartialEq)]
pub struct SceneInfo {
    pub id: Option<String>,
    pub component: String,
    /// Seconds; null while a scene's length is unknown (see `detail_in`).
    pub start: Option<f64>,
    pub duration: Option<f64>,
    pub text: Option<String>,
}

#[derive(Debug, Serialize, PartialEq)]
pub struct RenderFile {
    pub preset: Preset,
    pub path: String,
    pub modified: u64,
}

/// The video `slug` under `videos`. `settings_voice` is Settings -> Voice's
/// voice id, shown when video.json names none. Scene times are known once
/// every scene has a length: a hold, or narration that matches the script.
/// Until then they are null, and so is a stale narration's, which the page
/// meets as an error when it asks for a frame.
pub fn detail_in(
    videos: &Path,
    library: &Library,
    links: &HashMap<String, StudioLink>,
    settings_voice: Option<&str>,
    slug: &str,
) -> Result<VideoDetail, String> {
    let dir = videos::video_in(videos, slug)?;
    let row = videos::row(&dir);
    let project = Project::load_with(&dir, library).map_err(|e| e.to_string())?;
    let timeline = project.timeline().ok();
    let scenes = project
        .video
        .scenes
        .iter()
        .enumerate()
        .map(|(i, scene)| SceneInfo {
            id: scene.id.clone(),
            component: scene.component.clone(),
            start: timeline.as_ref().map(|t| t.start(i)),
            duration: timeline.as_ref().and_then(|t| t.scene_length(i)),
            text: project.section(i).map(|s| s.text.clone()),
        })
        .collect();
    let voice = project
        .video
        .voice
        .as_ref()
        .and_then(|v| v.voice.clone())
        .or_else(|| settings_voice.map(str::to_owned));
    Ok(VideoDetail {
        row: VideoView::of(row, links),
        fps: project.video.fps,
        width: project.video.size.width,
        height: project.video.size.height,
        theme: project.video.theme.clone(),
        voice,
        captions: project.video.captions,
        scenes,
        renders: renders(&dir),
    })
}

/// Each preset's file that exists, with when it was written.
fn renders(dir: &Path) -> Vec<RenderFile> {
    Preset::ALL
        .into_iter()
        .filter_map(|preset| {
            let path = preset.default_out(dir);
            let modified = path
                .metadata()
                .ok()?
                .modified()
                .ok()?
                .duration_since(UNIX_EPOCH)
                .ok()?
                .as_secs();
            Some(RenderFile {
                preset,
                path: path.to_string_lossy().into_owned(),
                modified,
            })
        })
        .collect()
}
