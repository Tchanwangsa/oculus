use std::path::Path;

use sqlx::{Row, SqlitePool};

pub async fn upsert_file(
    pool: &SqlitePool,
    subject_id: i64,
    relative_path: &str,
    size_bytes: u64,
    category: &str,
    canvas_id: Option<i64>,
    source_url: Option<&str>,
    changed: bool,
) -> Result<(), String> {
    let filename = relative_path
        .rsplit('/')
        .next()
        .unwrap_or(relative_path)
        .to_string();
    let file_type = filename
        .rsplit_once('.')
        .map(|(_, e)| e.to_string())
        .unwrap_or_else(|| "md".into());

    // `changed` is the engine's action; content_changed_at moves only on new bytes.
    sqlx::query(
        r#"INSERT INTO files (subject_id, filename, relative_path, file_type, size_bytes, category, canvas_id, source_url, first_seen_at, content_changed_at)
           VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, datetime('now'),
                   CASE WHEN ?9 THEN datetime('now') ELSE NULL END)
           ON CONFLICT(subject_id, relative_path) DO UPDATE SET
             filename   = excluded.filename,
             file_type  = excluded.file_type,
             size_bytes = excluded.size_bytes,
             category   = excluded.category,
             canvas_id  = excluded.canvas_id,
             source_url = excluded.source_url,
             scraped_at = datetime('now'),
             content_changed_at = CASE WHEN ?9 THEN excluded.content_changed_at ELSE files.content_changed_at END"#,
    )
    .bind(subject_id)
    .bind(&filename)
    .bind(relative_path)
    .bind(&file_type)
    .bind(size_bytes as i64)
    .bind(category)
    .bind(canvas_id)
    .bind(source_url)
    .bind(changed)
    .execute(pool)
    .await
    .map_err(|e| e.to_string())?;
    Ok(())
}

/// `files.id` for one artifact, needed to hang embedded pages off it.
pub async fn file_id(
    pool: &SqlitePool,
    subject_id: i64,
    relative_path: &str,
) -> Result<Option<i64>, String> {
    sqlx::query_scalar("SELECT id FROM files WHERE subject_id = ?1 AND relative_path = ?2")
        .bind(subject_id)
        .bind(relative_path)
        .fetch_optional(pool)
        .await
        .map_err(|e| e.to_string())
}

/// Write one file's page records.
///
/// The conflict clause is a CASE, not a `COALESCE`: an empty incoming page (a
/// full-bleed image, or a thinner re-parse) must leave good text alone. The
/// embedding columns are the embedder's and are never touched here.
pub async fn upsert_pages(
    pool: &SqlitePool,
    file_id: i64,
    pages: &[crate::parse::ParsePage],
) -> Result<usize, String> {
    let mut tx = pool.begin().await.map_err(|e| e.to_string())?;
    let with_text = write_pages(&mut tx, file_id, pages).await?;
    tx.commit().await.map_err(|e| e.to_string())?;
    Ok(with_text)
}

/// Replace one file's page records outright, vectors included: for text that
/// is never embedded (`crate::pages::sheets`), where a shorter workbook must lose
/// its old sheets.
pub async fn replace_pages(
    pool: &SqlitePool,
    file_id: i64,
    pages: &[crate::parse::ParsePage],
) -> Result<usize, String> {
    let mut tx = pool.begin().await.map_err(|e| e.to_string())?;
    sqlx::query("DELETE FROM pages WHERE file_id = ?1")
        .bind(file_id)
        .execute(&mut *tx)
        .await
        .map_err(|e| e.to_string())?;
    let with_text = write_pages(&mut tx, file_id, pages).await?;
    tx.commit().await.map_err(|e| e.to_string())?;
    Ok(with_text)
}

async fn write_pages(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    file_id: i64,
    pages: &[crate::parse::ParsePage],
) -> Result<usize, String> {
    let mut with_text = 0usize;
    for page in pages {
        if !page.markdown.is_empty() {
            with_text += 1;
        }
        sqlx::query(
            r#"INSERT INTO pages (file_id, page_no, markdown)
               VALUES (?1, ?2, ?3)
               ON CONFLICT(file_id, page_no) DO UPDATE SET
                 markdown = CASE WHEN excluded.markdown != '' THEN excluded.markdown ELSE pages.markdown END"#,
        )
        .bind(file_id)
        .bind(i64::from(page.page_no))
        .bind(&page.markdown)
        .execute(&mut **tx)
        .await
        .map_err(|e| format!("upsert page {}: {e}", page.page_no))?;
    }
    Ok(with_text)
}

/// How many page rows this file already has.
pub async fn page_count(pool: &SqlitePool, file_id: i64) -> Result<i64, String> {
    sqlx::query_scalar("SELECT COUNT(*) FROM pages WHERE file_id = ?1")
        .bind(file_id)
        .fetch_one(pool)
        .await
        .map_err(|e| e.to_string())
}

/// Every PDF-backed file on record (PDFs and Office documents with a derived
/// sibling PDF), optionally narrowed to a set of subjects. A file the user
/// skipped stays out until they parse it.
pub async fn pdf_files(
    pool: &SqlitePool,
    subject_ids: &[i64],
) -> Result<Vec<(i64, String)>, String> {
    let rows = sqlx::query(
        "SELECT subject_id, relative_path FROM files
         WHERE parse_status IS NULL OR parse_status != 'skipped'
         ORDER BY relative_path",
    )
    .fetch_all(pool)
    .await
    .map_err(|e| e.to_string())?;

    Ok(rows
        .iter()
        .map(|r| {
            (
                r.get::<i64, _>("subject_id"),
                r.get::<String, _>("relative_path"),
            )
        })
        .filter(|(sid, rel)| {
            crate::library::paths::doc_pdf_rel(rel).is_some()
                && (subject_ids.is_empty() || subject_ids.contains(sid))
        })
        .collect())
}

/// Derive parse status from what the parser left on disk: a record is
/// `quality`. Without one, the transient `queued`/`running` a killed run
/// leaves behind (and a `quality` whose record is gone) are cleared, but
/// `error` stays — it is the only trace of a failure from an earlier run —
/// and so does `skipped`, the only memory of a skip across restarts.
pub async fn reconcile_parse_status(pool: &SqlitePool, data_dir: &Path) -> Result<u64, String> {
    let rows = sqlx::query("SELECT relative_path FROM files")
        .fetch_all(pool)
        .await
        .map_err(|e| e.to_string())?;

    let mut updated = 0;
    for r in &rows {
        let rel: String = r.get("relative_path");
        let Some(pdf_rel) = crate::library::paths::doc_pdf_rel(&rel) else {
            continue;
        };

        let res = match crate::parse::parse_mode(&data_dir.join(&pdf_rel)) {
            Some(status) => {
                sqlx::query(
                    "UPDATE files SET parse_status = ?1, parsed_at = datetime('now')
                     WHERE relative_path = ?2 AND (parse_status IS NULL OR parse_status != ?1)",
                )
                .bind(status)
                .bind(&rel)
                .execute(pool)
                .await
            }
            None => {
                sqlx::query(
                    "UPDATE files SET parse_status = NULL, parsed_at = NULL
                     WHERE relative_path = ?1 AND parse_status IS NOT NULL
                       AND parse_status NOT IN ('error', 'skipped')",
                )
                .bind(&rel)
                .execute(pool)
                .await
            }
        }
        .map_err(|e| e.to_string())?;
        updated += res.rows_affected();
    }
    Ok(updated)
}

/// Chaptering runs killed mid-job (`running`, no `chaptered_at`), cleared at
/// startup: only a live process could clear the status otherwise.
pub async fn reconcile_chapter_status(pool: &SqlitePool) -> Result<u64, String> {
    sqlx::query(
        "UPDATE lectures SET chapter_status = NULL, chapter_error = NULL
          WHERE chapter_status = 'running'",
    )
    .execute(pool)
    .await
    .map(|r| r.rows_affected())
    .map_err(|e| e.to_string())
}

/// Lecture-end runs killed mid-job. The found end, if any, stays.
pub async fn reconcile_content_end_status(pool: &SqlitePool) -> Result<u64, String> {
    sqlx::query(
        "UPDATE lectures SET content_end_status = NULL, content_end_error = NULL
          WHERE content_end_status = 'running'",
    )
    .execute(pool)
    .await
    .map(|r| r.rows_affected())
    .map_err(|e| e.to_string())
}
