use sqlx::{Row, SqlitePool};

/// One Canvas page embed's Echo360 identity and what was fetched for it.
/// Paths are data-dir-relative.
#[derive(Debug, Clone, PartialEq)]
pub struct EchoEmbed {
    pub link_id: String,
    pub subject_id: i64,
    pub media_id: String,
    pub secure_link_id: String,
    pub name: String,
    pub duration_s: Option<f64>,
    pub restrict_download: bool,
    pub has_captions: bool,
    pub poster_rel: Option<String>,
    pub file_rel: Option<String>,
    pub extra_sources: i64,
}

pub async fn echo_embed(pool: &SqlitePool, link_id: &str) -> Result<Option<EchoEmbed>, String> {
    let row = sqlx::query(
        "SELECT link_id, subject_id, media_id, secure_link_id, name, duration_s,
                restrict_download, has_captions, poster_rel, file_rel, extra_sources
           FROM echo_embeds WHERE link_id = ?1",
    )
    .bind(link_id)
    .fetch_optional(pool)
    .await
    .map_err(|e| e.to_string())?;
    Ok(row.map(|r| EchoEmbed {
        link_id: r.get("link_id"),
        subject_id: r.get("subject_id"),
        media_id: r.get("media_id"),
        secure_link_id: r.get("secure_link_id"),
        name: r.get("name"),
        duration_s: r.get("duration_s"),
        restrict_download: r.get::<i64, _>("restrict_download") != 0,
        has_captions: r.get::<i64, _>("has_captions") != 0,
        poster_rel: r.get("poster_rel"),
        file_rel: r.get("file_rel"),
        extra_sources: r.get("extra_sources"),
    }))
}

/// Insert or refresh a link's row. `file_rel` is kept as it was: only
/// [`set_echo_embed_file`] changes it.
pub async fn upsert_echo_embed(pool: &SqlitePool, e: &EchoEmbed) -> Result<(), String> {
    sqlx::query(
        r#"INSERT INTO echo_embeds
             (link_id, subject_id, media_id, secure_link_id, name, duration_s,
              restrict_download, has_captions, poster_rel, file_rel, extra_sources, resolved_at)
           VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, datetime('now'))
           ON CONFLICT(link_id) DO UPDATE SET
             subject_id        = excluded.subject_id,
             media_id          = excluded.media_id,
             secure_link_id    = excluded.secure_link_id,
             name              = excluded.name,
             duration_s        = excluded.duration_s,
             restrict_download = excluded.restrict_download,
             has_captions      = excluded.has_captions,
             poster_rel        = COALESCE(excluded.poster_rel, echo_embeds.poster_rel),
             extra_sources     = excluded.extra_sources,
             resolved_at       = datetime('now')"#,
    )
    .bind(&e.link_id)
    .bind(e.subject_id)
    .bind(&e.media_id)
    .bind(&e.secure_link_id)
    .bind(&e.name)
    .bind(e.duration_s)
    .bind(e.restrict_download as i64)
    .bind(e.has_captions as i64)
    .bind(&e.poster_rel)
    .bind(&e.file_rel)
    .bind(e.extra_sources)
    .execute(pool)
    .await
    .map(|_| ())
    .map_err(|e| e.to_string())
}

pub async fn set_echo_embed_file(
    pool: &SqlitePool,
    link_id: &str,
    file_rel: Option<&str>,
) -> Result<(), String> {
    sqlx::query("UPDATE echo_embeds SET file_rel = ?1 WHERE link_id = ?2")
        .bind(file_rel)
        .bind(link_id)
        .execute(pool)
        .await
        .map(|_| ())
        .map_err(|e| e.to_string())
}

/// Where another link of the same subject already put this media, if any:
/// one recording embedded on two pages is downloaded once.
pub async fn echo_embed_file_of_media(
    pool: &SqlitePool,
    subject_id: i64,
    media_id: &str,
) -> Result<Vec<String>, String> {
    sqlx::query_scalar(
        "SELECT file_rel FROM echo_embeds
          WHERE subject_id = ?1 AND media_id = ?2 AND file_rel IS NOT NULL",
    )
    .bind(subject_id)
    .bind(media_id)
    .fetch_all(pool)
    .await
    .map_err(|e| e.to_string())
}
