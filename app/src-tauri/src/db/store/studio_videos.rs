//! `studio_videos`: which harness thread makes each Studio video, and the
//! length it was asked for. The folder under `<data>/studio/videos/` is the
//! video; a row here only links it to its thread.

use std::collections::HashMap;

use sqlx::{Row, SqlitePool};

/// What a video's thread was started with.
#[derive(Debug, Clone, PartialEq)]
pub struct StudioLink {
    pub slug: String,
    pub thread_id: Option<i64>,
    /// The narration length the student asked for, in seconds.
    pub length_secs: Option<i64>,
}

/// Link `slug` to `thread_id`, replacing a row a deleted video left behind.
pub async fn link_studio_video(
    pool: &SqlitePool,
    slug: &str,
    thread_id: i64,
    length_secs: Option<i64>,
) -> Result<(), String> {
    sqlx::query(
        "INSERT INTO studio_videos (slug, thread_id, length_secs) VALUES (?1, ?2, ?3)
         ON CONFLICT(slug) DO UPDATE SET
           thread_id   = excluded.thread_id,
           length_secs = excluded.length_secs,
           created_at  = datetime('now')",
    )
    .bind(slug)
    .bind(thread_id)
    .bind(length_secs)
    .execute(pool)
    .await
    .map(|_| ())
    .map_err(|e| e.to_string())
}

/// The video a thread makes, if it makes one.
pub async fn studio_link_of_thread(
    pool: &SqlitePool,
    thread_id: i64,
) -> Result<Option<StudioLink>, String> {
    let row =
        sqlx::query("SELECT slug, thread_id, length_secs FROM studio_videos WHERE thread_id = ?1")
            .bind(thread_id)
            .fetch_optional(pool)
            .await
            .map_err(|e| e.to_string())?;
    Ok(row.map(link))
}

/// Every link, by slug.
pub async fn studio_links(pool: &SqlitePool) -> Result<HashMap<String, StudioLink>, String> {
    let rows = sqlx::query("SELECT slug, thread_id, length_secs FROM studio_videos")
        .fetch_all(pool)
        .await
        .map_err(|e| e.to_string())?;
    Ok(rows
        .into_iter()
        .map(link)
        .map(|l| (l.slug.clone(), l))
        .collect())
}

fn link(r: sqlx::sqlite::SqliteRow) -> StudioLink {
    StudioLink {
        slug: r.get("slug"),
        thread_id: r.get("thread_id"),
        length_secs: r.get("length_secs"),
    }
}
