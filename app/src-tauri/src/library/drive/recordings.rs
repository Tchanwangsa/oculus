//! Lecture recordings in a listing: a `lectures` row stands in for its
//! folder, so the folder opens the lecture page instead of its media files.

use std::path::Path;

use sqlx::{Row, SqlitePool};

use super::listing::data_rel;
use crate::library::layout;
use crate::library::paths::fold_code;

/// One lecture and the folder its recording lives in.
#[derive(Clone, Debug)]
pub struct Recording {
    pub lecture_id: String,
    pub subject_id: i64,
    pub title: String,
    pub date: String,
    /// Data-dir-relative, as the layout names it (`layout::recording_dir`).
    pub folder_rel: String,
}

/// The lectures of the subject whose folder is `folder` (`courses/<folder>`),
/// each with its recording's folder.
pub async fn subject_recordings(
    pool: &SqlitePool,
    data_dir: &Path,
    folder: &str,
) -> Result<Vec<Recording>, String> {
    let rows = sqlx::query(
        "SELECT l.id, l.subject_id, l.title, l.date, s.code
         FROM lectures l JOIN subjects s ON s.id = l.subject_id",
    )
    .fetch_all(pool)
    .await
    .map_err(|e| e.to_string())?;
    let wanted = fold_code(folder);
    Ok(rows
        .iter()
        .filter(|r| {
            let code: String = r.get("code");
            fold_code(&crate::library::paths::safe_dir(&code)) == wanted
        })
        .filter_map(|r| {
            let lecture_id: String = r.get("id");
            let dir = layout::recording_dir(data_dir, &lecture_id).ok()?;
            Some(Recording {
                folder_rel: data_rel(data_dir, &dir)?,
                subject_id: r.get("subject_id"),
                title: r.get("title"),
                date: r.get("date"),
                lecture_id,
            })
        })
        .collect())
}
