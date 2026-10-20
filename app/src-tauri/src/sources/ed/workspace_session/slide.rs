//! The slide a challenge belongs to, found in the library by the
//! `ed_challenge_id` in its front matter, and the challenge snapshot the
//! sync wrote beside it (`challenge`, `docs/sync.md`).

use std::io::Read;
use std::path::Path;

use serde_json::Value;

use super::events::Features;
use crate::library::paths::slash_path;
use crate::sources::ed::{front::read_front_matter, slide_workspace_dir, workspace_state};

/// Front matter sits in a slide file's first lines; the rest is not read.
const HEAD_BYTES: u64 = 16 * 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Slide {
    /// The workspace folder, data-dir-relative.
    pub slide_dir: String,
    /// The lesson slide's Ed id (`ed_slide_id`), which a passing Mark completes.
    pub slide_id: Option<i64>,
    pub run_command: Option<String>,
    pub features: Features,
    pub due_at: Option<String>,
}

/// The challenge's slide: the one its sync record names when that still
/// holds it, else the first under `courses/*/lessons/*/` that does.
pub fn find(data_dir: &Path, challenge_id: i64) -> Result<Slide, String> {
    let recorded = workspace_state::load(data_dir, challenge_id)
        .map(|s| s.slide_dir)
        .filter(|d| !d.is_empty())
        .and_then(|d| read(data_dir, &format!("{d}.md"), challenge_id));
    if let Some(slide) = recorded {
        return Ok(slide);
    }
    for md in slide_files(data_dir) {
        if let Some(slide) = read(data_dir, &md, challenge_id) {
            return Ok(slide);
        }
    }
    Err(format!(
        "No slide in the library holds Ed challenge {challenge_id}: sync its subject first"
    ))
}

/// Every lesson slide file, data-dir-relative, sorted.
fn slide_files(data_dir: &Path) -> Vec<String> {
    let mut found = Vec::new();
    for course in dirs(&data_dir.join("courses")) {
        for lesson in dirs(&course.join("lessons")) {
            let Ok(entries) = std::fs::read_dir(&lesson) else {
                continue;
            };
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().is_some_and(|e| e == "md") && path.is_file() {
                    if let Ok(rel) = path.strip_prefix(data_dir) {
                        found.push(slash_path(rel));
                    }
                }
            }
        }
    }
    found.sort();
    found
}

fn dirs(parent: &Path) -> Vec<std::path::PathBuf> {
    let mut dirs: Vec<_> = std::fs::read_dir(parent)
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.is_dir())
        .collect();
    dirs.sort();
    dirs
}

/// The slide at `md` (data-dir-relative), when it is challenge `challenge_id`'s.
fn read(data_dir: &Path, md: &str, challenge_id: i64) -> Option<Slide> {
    let mut head = String::new();
    std::fs::File::open(data_dir.join(md))
        .ok()?
        .take(HEAD_BYTES)
        .read_to_string(&mut head)
        .ok()?;
    if !head.contains(&format!("ed_challenge_id: {challenge_id}\n")) {
        return None;
    }
    let (front, _) = read_front_matter(&head)?;
    if front.get("ed_challenge_id")?.as_i64()? != challenge_id {
        return None;
    }
    Some(snapshot(
        slide_workspace_dir(md)?,
        front.get("ed_slide_id").and_then(Value::as_i64),
        &front["challenge"],
    ))
}

/// The sync's `challenge` snapshot; a missing one (its read failed) leaves
/// every feature off.
fn snapshot(slide_dir: &str, slide_id: Option<i64>, challenge: &Value) -> Slide {
    let feature = |name: &str| challenge["features"][name].as_bool().unwrap_or(false);
    let text = |v: &Value| v.as_str().filter(|s| !s.is_empty()).map(str::to_string);
    Slide {
        slide_dir: slide_dir.to_string(),
        slide_id,
        run_command: text(&challenge["run_command"]),
        features: Features {
            run: feature("run"),
            mark: feature("mark"),
            terminal: feature("terminal"),
            feedback: feature("feedback"),
            confirm_submit: feature("confirm_submit"),
        },
        due_at: text(&challenge["due_at"]),
    }
}
