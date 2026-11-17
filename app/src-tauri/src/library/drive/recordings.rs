//! Lecture recordings in a listing: a `lectures` row stands in for its
//! folder, so the folder opens the lecture page instead of its media files.

use std::path::Path;

use sqlx::{Row, SqlitePool};

use crate::library::layout;
use crate::library::paths::fold_code;

/// One lecture and the folder its recording lives in.
#[derive(Clone, Debug)]
pub struct Recording {
    pub lecture_id: String,
    pub subject_id: i64,
    pub title: String,
    pub date: String,
    /// Data-dir-relative, as the row keeps it (`lectures.folder_rel`).
    pub folder_rel: String,
}

/// The lectures of the subject whose folder is `folder` (`courses/<folder>`),
/// each with its recording's folder; a lecture with no folder yet (nothing of
/// it downloaded) has nothing on disk to stand in for.
pub async fn subject_recordings(
    pool: &SqlitePool,
    data_dir: &Path,
    folder: &str,
) -> Result<Vec<Recording>, String> {
    let rows = sqlx::query(
        "SELECT l.id, l.subject_id, l.title, l.date, l.folder_rel, s.code
         FROM lectures l JOIN subjects s ON s.id = l.subject_id
         WHERE l.folder_rel IS NOT NULL",
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
            let folder_rel: String = r.get("folder_rel");
            layout::recording_dir(data_dir, &folder_rel).ok()?;
            Some(Recording {
                folder_rel,
                subject_id: r.get("subject_id"),
                title: r.get("title"),
                date: r.get("date"),
                lecture_id: r.get("id"),
            })
        })
        .collect())
}
