//! A video's thumbnail: a still of its first scene, cached on disk.
//!
//! A narrated video shows scene 0 about a second in. One whose narration is
//! missing or stale has no scene lengths to draw by, so its thumbnail is
//! scene 0 drawn silent at the end of its hold, from a copy of the project
//! with the script and manifest taken off; a scene with no hold gets
//! `SILENT_HOLD`. The PNG is `<video>/.cache/thumbs/<key>.png` (`still_key`);
//! writing one deletes the others, and `renders/` is never touched.

use std::path::Path;

use studio_core::{Library, Project, Time};

use super::frame::png;
use crate::studio::render::still_key;
use crate::studio::videos;

/// The longer side of the PNG, in px.
const LONG_SIDE: f64 = 640.0;
/// How far into a narrated scene 0 the still is taken.
const NARRATED_AT: f64 = 1.0;
/// The hold given to a silent scene that has none.
const SILENT_HOLD: f64 = 3.0;

/// The thumbnail of video `slug` under `videos`, drawn if its cached file is
/// missing. A failure names the video and why: the page shows it in place
/// of a picture.
pub fn thumbnail_in(videos: &Path, library: &Library, slug: &str) -> Result<Vec<u8>, String> {
    let dir = videos::video_in(videos, slug)?;
    let fail = |e: String| format!("no thumbnail for `{slug}`: {e}");
    let loaded = Project::load_with(&dir, library).map_err(|e| fail(e.to_string()))?;
    let (project, t, silent) = match loaded.timeline() {
        Ok(_) => {
            let t = still_time(&loaded, NARRATED_AT).map_err(fail)?;
            (loaded, t, false)
        }
        Err(_) => {
            let silent = silenced(loaded);
            let t = still_time(&silent, f64::INFINITY).map_err(fail)?;
            (silent, t, true)
        }
    };
    let size = project.video.size;
    let scale = (LONG_SIDE / f64::from(size.width.max(size.height))).min(1.0);
    let settings = format!("thumbnail 1 scale={scale} t={t} silent={silent}");
    let key = still_key(&dir, &project, 0, &settings).map_err(|e| fail(e.to_string()))?;

    let folder = dir.join(".cache").join("thumbs");
    let file = folder.join(format!("{key}.png"));
    if let Ok(bytes) = std::fs::read(&file) {
        return Ok(bytes);
    }
    let bytes = png(&project, Time::Scene { index: 0, t }, scale).map_err(fail)?;
    store(&folder, &file, &bytes).map_err(fail)?;
    Ok(bytes)
}

/// `project` with no script and no manifest, every scene given a hold: it
/// draws as a silent video.
fn silenced(mut project: Project) -> Project {
    project.script = None;
    project.manifest = None;
    for scene in &mut project.video.scenes {
        scene.hold.get_or_insert(SILENT_HOLD);
    }
    project
}

/// `want` seconds into scene 0, or its last whole frame if it is shorter.
fn still_time(project: &Project, want: f64) -> Result<f64, String> {
    let length = project.scene_length(0).map_err(|e| e.to_string())?;
    let last = (length - 1.0 / f64::from(project.fps())).max(0.0);
    Ok(want.min(last))
}

/// Write `bytes` to `file`, then delete every other file in `folder`.
fn store(folder: &Path, file: &Path, bytes: &[u8]) -> Result<(), String> {
    std::fs::create_dir_all(folder)
        .map_err(|e| format!("cannot create {}: {e}", folder.display()))?;
    let tmp = file.with_extension(format!(
        "png.{}-{}.part",
        std::process::id(),
        crate::runtime::clock::now_nanos()
    ));
    crate::runtime::atomic_write::write(file, &tmp, bytes)?;
    for entry in std::fs::read_dir(folder).into_iter().flatten().flatten() {
        let path = entry.path();
        if path != file && path.is_file() {
            std::fs::remove_file(&path).ok();
        }
    }
    Ok(())
}
