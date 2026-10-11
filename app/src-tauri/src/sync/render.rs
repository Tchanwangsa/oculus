//! Pure helpers: slugs, paths inside a course, and Markdown metadata lines.

use std::collections::HashSet;

use crate::sync::Fetched;

pub(super) fn items_of(module: &serde_json::Value) -> &[serde_json::Value] {
    module["items"].as_array().map(Vec::as_slice).unwrap_or(&[])
}

pub(super) fn content_type_of(info: &serde_json::Value) -> String {
    info["content-type"]
        .as_str()
        .or_else(|| info["content_type"].as_str())
        .unwrap_or("")
        .split(';')
        .next()
        .unwrap_or("")
        .trim()
        .to_string()
}

/// The name a Canvas file is stored under, before path sanitising.
pub(super) fn file_name(info: &serde_json::Value, display: Option<&str>) -> String {
    info["filename"]
        .as_str()
        .or_else(|| info["display_name"].as_str())
        .or(display)
        .unwrap_or("file.bin")
        .replace(['/', '\\'], "_")
}

/// `Some(" until 2026-08-01")` (or `Some("")`) when Canvas lists the file
/// but refuses its download — checked explicitly, or it reads as an auth
/// failure.
pub(super) fn locked_until(info: &serde_json::Value) -> Option<String> {
    if !info["locked_for_user"].as_bool().unwrap_or(false) {
        return None;
    }
    Some(
        info["lock_info"]["unlock_at"]
            .as_str()
            .or_else(|| info["unlock_at"].as_str())
            .map(|d| format!(" until {}", &d[..10.min(d.len())]))
            .unwrap_or_default(),
    )
}

pub(super) fn modified_of(info: &serde_json::Value) -> String {
    info["modified_at"]
        .as_str()
        .or_else(|| info["updated_at"].as_str())
        .unwrap_or("")
        .to_string()
}

/// One module-TOC line for a file item; TOCs live in `modules/`, so links
/// step up a level. A video carries its Canvas id — `_(video <id>)_` — for
/// the app's on-demand download, and links where that download lands.
pub(super) fn file_toc_line(indent: &str, title: &str, fetched: &Fetched) -> String {
    match fetched {
        Fetched::Saved(rel) => format!(
            "{indent}- [{}](../{})",
            escape_md(title),
            rel_within_course(rel)
        ),
        Fetched::Video { rel, canvas_id } => format!(
            "{indent}- [{}](../{}) _(video {canvas_id})_",
            escape_md(title),
            rel_within_course(rel)
        ),
        Fetched::Skipped => format!("{indent}- {} _(file)_", escape_md(title)),
    }
}

/// `courses/CODE/files/x.pdf` → `files/x.pdf`.
pub(super) fn rel_within_course(rel: &str) -> String {
    rel.splitn(3, '/').nth(2).unwrap_or(rel).to_string()
}

/// The `../` prefix a document at `out_path` needs to reach the course root.
pub(super) fn up_to_course_root(out_path: &str) -> String {
    "../".repeat(out_path.matches('/').count())
}

/// `<dir>/<slug>.md`, or `<slug>-<id>.md` when two titles slug identically.
pub(super) fn task_path(dir: &str, title: &str, id: i64, used: &mut HashSet<String>) -> String {
    let base = format!("{dir}/{}", slug(title));
    if used.insert(base.clone()) {
        format!("{base}.md")
    } else {
        format!("{base}-{id}.md")
    }
}

/// `"submitted"`/`"graded"` when the user has handed the task in. From
/// `include[]=submission` on the assignments API.
pub(super) fn submission_status(item: &serde_json::Value) -> Option<&'static str> {
    match item["submission"]["workflow_state"].as_str() {
        Some("graded") => Some("graded"),
        Some("submitted") | Some("pending_review") => Some("submitted"),
        _ => None,
    }
}

/// Append `**Label:** <timestamp>` when Canvas supplied one.
pub(super) fn push_ts(meta: &mut Vec<String>, label: &str, iso: Option<&str>) {
    if let Some(ts) = iso.filter(|s| !s.is_empty()) {
        meta.push(format!("**{label}:** {}", fmt_ts(ts)));
    }
}

/// `"2026-09-12T13:59:59Z"` → `"2026-09-12 13:59 UTC"`; left in UTC.
fn fmt_ts(iso: &str) -> String {
    if iso.len() >= 16 && iso.as_bytes()[10] == b'T' {
        format!("{} {} UTC", &iso[..10], &iso[11..16])
    } else {
        iso.to_string()
    }
}

/// `20.0` → `"20"`, `12.5` → `"12.5"`.
pub(super) fn fmt_points(p: f64) -> String {
    if p.fract() == 0.0 {
        format!("{}", p as i64)
    } else {
        format!("{p}")
    }
}

pub(super) fn escape_md(s: &str) -> String {
    s.chars()
        .flat_map(|c| {
            let esc = matches!(c, '*' | '_' | '`' | '[' | ']' | '\\');
            esc.then_some('\\').into_iter().chain(std::iter::once(c))
        })
        .collect()
}

/// Filename-safe slug, capped so the filesystem never rejects the path.
pub fn slug(s: &str) -> String {
    let mut out = String::new();
    let mut pending_dash = false;
    for c in s.to_lowercase().chars() {
        if c.is_ascii_alphanumeric() {
            if pending_dash && !out.is_empty() {
                out.push('-');
            }
            pending_dash = false;
            out.push(c);
        } else {
            pending_dash = true;
        }
    }
    out.truncate(60);
    if out.is_empty() {
        "untitled".to_string()
    } else {
        out
    }
}
