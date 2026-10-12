//! The Videos list: each folder's `videos::VideoRow` with a title and the
//! thread that made it.

use std::collections::HashMap;
use std::path::Path;

use serde::Serialize;

use crate::db::store::StudioLink;
use crate::studio::videos::{self, VideoRow};

/// Mirrors `VideoRow` in `app/src/lib/studio/videos.ts`.
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct VideoView {
    pub slug: String,
    pub dir: String,
    pub title: String,
    pub subject: Option<String>,
    pub scenes: Option<usize>,
    pub narrated: bool,
    pub renders: Vec<String>,
    pub modified: Option<u64>,
    pub problem: Option<String>,
    pub thread_id: Option<i64>,
}

impl VideoView {
    /// `links` is the whole link table, by slug. video.json has no title
    /// field, so the title is the slug, spelled out (`videos::title_of`).
    pub fn of(row: VideoRow, links: &HashMap<String, StudioLink>) -> Self {
        Self {
            title: videos::title_of(&row.slug),
            thread_id: links.get(&row.slug).and_then(|l| l.thread_id),
            dir: row.dir.to_string_lossy().into_owned(),
            slug: row.slug,
            subject: row.subject,
            scenes: row.scenes,
            narrated: row.narrated,
            renders: row.renders,
            modified: row.modified,
            problem: row.problem,
        }
    }
}

/// The videos under `videos`, newest change first (a video whose video.json
/// does not load sorts last), optionally one subject's.
pub fn list_in(
    videos: &Path,
    subject: Option<&str>,
    links: &HashMap<String, StudioLink>,
) -> Result<Vec<VideoView>, String> {
    let mut rows: Vec<VideoView> = videos::list_in(videos, subject)?
        .into_iter()
        .map(|row| VideoView::of(row, links))
        .collect();
    rows.sort_by(|a, b| {
        b.modified
            .cmp(&a.modified)
            .then_with(|| a.slug.cmp(&b.slug))
    });
    Ok(rows)
}
