//! Where videos live: `<data dir>/studio/videos/<slug>/`, one folder per
//! video. `create` scaffolds one, `list` reads them all, and `resolve` turns
//! a command's DIR argument into a folder: a path that exists wins, else a
//! bare slug names a video here (docs/studio.md, "Videos live in the data
//! directory").

#[cfg(test)]
mod tests;

use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use serde::Serialize;
use serde_json::json;
use studio_core::{valid_subject, Video};

use crate::library::paths::{data_dir, studio_dir};

/// `<data dir>/studio/videos`.
pub fn videos_dir() -> PathBuf {
    studio_dir(&data_dir()).join("videos")
}

/// A slug is lowercase letters, digits and `-`, at most 64 characters.
pub fn valid_slug(slug: &str) -> bool {
    !slug.is_empty()
        && slug.len() <= 64
        && slug
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
}

/// [`resolve_in`] over the app's videos.
pub fn resolve(arg: &Path) -> Result<PathBuf, String> {
    resolve_in(&videos_dir(), arg)
}

/// `arg` when it exists; otherwise, when it is a bare slug, that video's
/// folder under `videos`. Anything else is an error naming both places.
pub fn resolve_in(videos: &Path, arg: &Path) -> Result<PathBuf, String> {
    let exists = arg
        .try_exists()
        .map_err(|e| format!("cannot look at {}: {e}", arg.display()))?;
    if exists {
        return Ok(arg.to_owned());
    }
    let Some(slug) = arg.to_str().filter(|s| valid_slug(s)) else {
        return Err(format!("no folder {}", arg.display()));
    };
    let dir = videos.join(slug);
    if dir.is_dir() {
        return Ok(dir);
    }
    Err(format!(
        "no folder `{slug}` here and no video `{slug}` in {}; `oculus studio list` lists the videos",
        videos.display()
    ))
}

/// What `oculus studio new` makes.
#[derive(Debug, Clone, Default)]
pub struct NewVideo {
    pub slug: String,
    pub subject: Option<String>,
    pub title: Option<String>,
    /// 1080x1920 instead of 1920x1080.
    pub portrait: bool,
}

/// Scaffolds `videos/<slug>/`: a `video.json` (30 fps, theme `default`,
/// which a subject's own `themes/default.json` replaces through the
/// cascade, the subject, and one silent `TitleCard` scene), an empty
/// `script.md`, and empty `components/` and `assets/`. Refuses a slug that
/// exists. Returns the folder.
pub fn create_in(videos: &Path, new: &NewVideo) -> Result<PathBuf, String> {
    if !valid_slug(&new.slug) {
        return Err(format!(
            "slug `{}` must be lowercase letters, digits and `-`, at most 64 characters",
            new.slug
        ));
    }
    if let Some(s) = new.subject.as_deref().filter(|s| !valid_subject(s)) {
        return Err(format!(
            "subject `{s}` must be a course code's folder name: letters, digits, `-` and `_`"
        ));
    }
    std::fs::create_dir_all(videos)
        .map_err(|e| format!("cannot create {}: {e}", videos.display()))?;
    let dir = videos.join(&new.slug);
    match std::fs::create_dir(&dir) {
        Ok(()) => {}
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
            return Err(format!(
                "video `{}` exists at {}; pick another slug",
                new.slug,
                dir.display()
            ))
        }
        Err(e) => return Err(format!("cannot create {}: {e}", dir.display())),
    }
    for sub in ["components", "assets"] {
        let path = dir.join(sub);
        std::fs::create_dir(&path).map_err(|e| format!("cannot create {}: {e}", path.display()))?;
    }
    let file = |name: &str, text: &str| {
        let path = dir.join(name);
        std::fs::write(&path, text).map_err(|e| format!("cannot write {}: {e}", path.display()))
    };
    file("video.json", &video_json(new))?;
    file("script.md", "")?;
    Ok(dir)
}

/// The scaffold's `video.json`, keys in reading order.
fn video_json(new: &NewVideo) -> String {
    let (width, height) = if new.portrait {
        (1080, 1920)
    } else {
        (1920, 1080)
    };
    let title = new.title.clone().unwrap_or_else(|| title_of(&new.slug));
    let scene = json!({
        "id": "title",
        "component": "TitleCard",
        "props": { "title": title },
        "hold": 4,
    });
    let mut lines = vec![
        format!("  \"size\": {{ \"width\": {width}, \"height\": {height} }},"),
        "  \"fps\": 30,".to_owned(),
        "  \"theme\": \"default\",".to_owned(),
    ];
    if let Some(s) = &new.subject {
        lines.push(format!("  \"subject\": {},", json!(s)));
    }
    lines.push(format!("  \"scenes\": [\n    {scene}\n  ]"));
    format!("{{\n{}\n}}\n", lines.join("\n"))
}

/// `gradient-descent` -> `Gradient descent`.
fn title_of(slug: &str) -> String {
    let words = slug.replace('-', " ");
    let mut chars = words.trim().chars();
    match chars.next() {
        Some(first) => first.to_uppercase().chain(chars).collect(),
        None => String::new(),
    }
}

/// One video, as `oculus studio list` shows it.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct VideoRow {
    pub slug: String,
    pub dir: PathBuf,
    pub subject: Option<String>,
    /// `None` when video.json does not load; `problem` says why.
    pub scenes: Option<usize>,
    /// `narration/manifest.json` exists (it may still be stale; `lint` says).
    pub narrated: bool,
    /// The files in `renders/`, e.g. `draft.mp4`.
    pub renders: Vec<String>,
    /// The newest change to video.json, script.md or `components/`, as Unix
    /// seconds.
    pub modified: Option<u64>,
    pub problem: Option<String>,
}

/// Every folder under `videos`, by slug; with `subject`, only that
/// subject's. A folder whose video.json does not load is listed with the
/// reason (and is never one `subject` matches).
pub fn list_in(videos: &Path, subject: Option<&str>) -> Result<Vec<VideoRow>, String> {
    let entries = match std::fs::read_dir(videos) {
        Ok(entries) => entries,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(format!("cannot read {}: {e}", videos.display())),
    };
    let mut rows = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|e| format!("cannot read {}: {e}", videos.display()))?;
        let dir = entry.path();
        if !dir.is_dir() {
            continue;
        }
        let row = row(&dir);
        if subject.is_none_or(|s| row.subject.as_deref() == Some(s)) {
            rows.push(row);
        }
    }
    rows.sort_by(|a, b| a.slug.cmp(&b.slug));
    Ok(rows)
}

fn row(dir: &Path) -> VideoRow {
    let slug = dir
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let path = dir.join("video.json");
    let video = std::fs::read_to_string(&path)
        .map_err(|e| format!("cannot read {}: {e}", path.display()))
        .and_then(|text| {
            serde_json::from_str::<Video>(&text).map_err(|e| format!("{}: {e}", path.display()))
        });
    let (subject, scenes, problem) = match video {
        Ok(v) => (v.subject, Some(v.scenes.len()), None),
        Err(e) => (None, None, Some(e)),
    };
    let mut renders: Vec<String> = std::fs::read_dir(dir.join("renders"))
        .map(|rd| {
            rd.flatten()
                .filter(|e| e.path().is_file())
                .map(|e| e.file_name().to_string_lossy().into_owned())
                .collect()
        })
        .unwrap_or_default();
    renders.sort();
    VideoRow {
        slug,
        dir: dir.to_owned(),
        subject,
        scenes,
        narrated: dir.join(studio_core::MANIFEST_PATH).is_file(),
        renders,
        modified: modified(dir),
        problem,
    }
}

fn modified(dir: &Path) -> Option<u64> {
    let components = std::fs::read_dir(dir.join("components"))
        .map(|rd| rd.flatten().map(|e| e.path()).collect::<Vec<_>>())
        .unwrap_or_default();
    [dir.join("video.json"), dir.join("script.md")]
        .into_iter()
        .chain(components)
        .filter_map(|p| p.metadata().and_then(|m| m.modified()).ok())
        .filter_map(|t| t.duration_since(UNIX_EPOCH).ok())
        .map(|d| d.as_secs())
        .max()
}
