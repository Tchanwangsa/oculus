//! What the plan reads from the database: the lectures to name recording
//! folders from, and which files in the old `Files/` are videos. Read before
//! the schema is brought up, so a column a later migration adds is optional.

use std::collections::HashSet;

use sqlx::{Row, SqlitePool};

use crate::library::layout;

pub struct LectureRow {
    pub id: String,
    pub code: Option<String>,
    pub title: String,
    pub date: String,
    pub folder_rel: Option<String>,
}

#[derive(Default)]
pub struct Rows {
    pub lectures: Vec<LectureRow>,
    /// Data-dir-relative paths of videos in a subject's old `Files/`.
    pub videos: HashSet<String>,
    /// Recording folders lectures already hold, lowercased.
    pub taken_folders: HashSet<String>,
    /// Every `files` row: id, subject, path.
    pub files: Vec<(i64, i64, String)>,
}

/// Extensions of the videos the sync files as a module's or a page's media.
/// `crate::sync::VIDEO_EXTS` and the embed downloader's list, together.
const VIDEO_EXTS: [&str; 6] = ["mp4", "m4v", "mov", "webm", "mkv", "avi"];

pub fn is_video(path: &str) -> bool {
    path.rsplit_once('.')
        .is_some_and(|(_, ext)| VIDEO_EXTS.contains(&ext.to_ascii_lowercase().as_str()))
}

async fn has_column(pool: &SqlitePool, table: &str, column: &str) -> Result<bool, String> {
    let names: Vec<String> = sqlx::query_scalar("SELECT name FROM pragma_table_info(?1)")
        .bind(table)
        .fetch_all(pool)
        .await
        .map_err(|e| e.to_string())?;
    Ok(names.iter().any(|n| n == column))
}

pub async fn rows(pool: &SqlitePool) -> Result<Rows, String> {
    let folder = if has_column(pool, "lectures", "folder_rel").await? {
        "l.folder_rel"
    } else {
        "NULL"
    };
    let lectures = sqlx::query(&format!(
        "SELECT l.id, s.code, l.title, l.date, {folder} AS folder_rel
           FROM lectures l LEFT JOIN subjects s ON s.id = l.subject_id
          ORDER BY l.date, l.id"
    ))
    .fetch_all(pool)
    .await
    .map_err(|e| format!("read lectures: {e}"))?
    .into_iter()
    .map(|r| LectureRow {
        id: r.get("id"),
        code: r.get("code"),
        title: r.get("title"),
        date: r.get("date"),
        folder_rel: r.get("folder_rel"),
    })
    .collect::<Vec<_>>();
    let taken_folders = lectures
        .iter()
        .filter_map(|l| l.folder_rel.as_ref().map(|f| f.to_lowercase()))
        .collect();

    let files_dir = layout::legacy_files_dir();
    let mut videos = HashSet::new();
    let files: Vec<(i64, i64, String)> =
        sqlx::query_as("SELECT id, subject_id, relative_path FROM files ORDER BY id")
            .fetch_all(pool)
            .await
            .map_err(|e| format!("read files: {e}"))?;
    let paths: Vec<String> = files.iter().map(|(_, _, p)| p.clone()).collect();
    let embeds: Vec<Option<String>> = sqlx::query_scalar("SELECT file_rel FROM echo_embeds")
        .fetch_all(pool)
        .await
        .map_err(|e| format!("read echo_embeds: {e}"))?;
    for path in paths.into_iter().chain(embeds.into_iter().flatten()) {
        let in_files = layout::split_course(&path)
            .is_some_and(|(_, inner)| inner.rsplit_once('/').is_some_and(|(d, _)| d == files_dir));
        if in_files && is_video(&path) {
            videos.insert(path);
        }
    }
    Ok(Rows {
        lectures,
        videos,
        taken_folders,
        files,
    })
}
