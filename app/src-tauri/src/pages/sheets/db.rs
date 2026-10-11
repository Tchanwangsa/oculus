//! Recording a converted sheet's pages in the database and reconciling the library.

use std::path::Path;

use sqlx::{Row, SqlitePool};

use crate::library::paths;
use crate::parse::{self, ParseError, ParsePage};

use super::{convert, needs_conversion, pdf_route_files};

/// Store a converted sheet's pages and mark it finished, as a parse would.
/// Its pages are replaced outright and it has no vectors, so the embed
/// columns are cleared.
///
/// A sync converts straight after its `scrape-file` event, which can beat the
/// frontend's write of the row, so a bare row is inserted here when there is
/// none; the frontend's upsert fills in the rest.
pub async fn record(
    pool: &SqlitePool,
    subject_id: i64,
    rel: &str,
    pages: &[ParsePage],
) -> Result<usize, String> {
    let filename = rel.rsplit('/').next().unwrap_or(rel);
    let file_type = filename.rsplit_once('.').map(|(_, e)| e).unwrap_or("");
    sqlx::query(
        "INSERT INTO files (subject_id, filename, relative_path, file_type, first_seen_at)
         VALUES (?1, ?2, ?3, ?4, datetime('now'))
         ON CONFLICT(subject_id, relative_path) DO NOTHING",
    )
    .bind(subject_id)
    .bind(filename)
    .bind(rel)
    .bind(file_type)
    .execute(pool)
    .await
    .map_err(|e| e.to_string())?;
    let Some(file_id) = crate::db::store::file_id(pool, subject_id, rel).await? else {
        return Ok(0);
    };
    let recorded = crate::db::store::replace_pages(pool, file_id, pages).await?;
    sqlx::query(
        "UPDATE files SET parse_status = 'quality', parsed_at = datetime('now'),
                          embed_status = NULL, embedded_at = NULL
          WHERE id = ?1",
    )
    .bind(file_id)
    .execute(pool)
    .await
    .map_err(|e| e.to_string())?;
    Ok(recorded)
}

/// A sheet that did not convert has no text: its pages go and it reads as
/// failed until its bytes change or it is retried.
pub(super) async fn forget(pool: &SqlitePool, subject_id: i64, rel: &str) -> Result<(), String> {
    let Some(file_id) = crate::db::store::file_id(pool, subject_id, rel).await? else {
        return Ok(());
    };
    sqlx::query("DELETE FROM pages WHERE file_id = ?1")
        .bind(file_id)
        .execute(pool)
        .await
        .map_err(|e| e.to_string())?;
    sqlx::query(
        "UPDATE files SET parse_status = 'error', parsed_at = NULL,
                          embed_status = NULL, embedded_at = NULL
          WHERE id = ?1",
    )
    .bind(file_id)
    .execute(pool)
    .await
    .map_err(|e| e.to_string())?;
    Ok(())
}

/// Convert, record and report one spreadsheet, blocking (it takes moments).
/// Ends in the `parse-status` a parse ends in — `quality`, or `error` with a
/// `Document` failure — so the pipeline row settles; no parse is started.
/// Call from a plain thread: it blocks on the async runtime.
pub fn index(data_dir: &Path, rel: &str, subject_id: i64) -> Result<usize, ParseError> {
    let converted = convert(data_dir, rel);
    let stored = tauri::async_runtime::block_on(async {
        let pool = crate::db::store::open(data_dir).await?;
        let result = match &converted {
            Ok(pages) => record(&pool, subject_id, rel, pages).await,
            Err(_) => forget(&pool, subject_id, rel).await.map(|()| 0),
        };
        pool.close().await;
        result
    });
    // A database problem must not fail a conversion that is on disk; the
    // startup reconcile records it later.
    let recorded = stored.unwrap_or_else(|e| {
        eprintln!("[oculus] spreadsheet {rel}: not recorded: {e}");
        0
    });
    match converted {
        Ok(_) => {
            parse::events::parsed(rel, subject_id);
            Ok(recorded)
        }
        Err(error) => {
            parse::events::failed(rel, subject_id, &error);
            Err(error)
        }
    }
}

/// Bring every spreadsheet on record to its text: one with no text, PDF-route
/// files beside it, no pages, vectors or an unfinished status is converted
/// again. A failure stays failed unless PDF-route files are still there, and
/// a skip stays skipped.
/// Silent — the frontend reads the rows. Returns how many were converted.
pub async fn reconcile(pool: &SqlitePool, data_dir: &Path) -> Result<u64, String> {
    let rows = sqlx::query(
        "SELECT f.subject_id, f.relative_path, f.parse_status, f.embed_status,
                (SELECT COUNT(*) FROM pages p WHERE p.file_id = f.id) AS pages
           FROM files f",
    )
    .fetch_all(pool)
    .await
    .map_err(|e| e.to_string())?;

    let mut converted = 0;
    for row in &rows {
        let rel: String = row.get("relative_path");
        if !paths::is_sheet(&rel) || !data_dir.join(&rel).is_file() {
            continue;
        }
        let subject_id: i64 = row.get("subject_id");
        let status: Option<String> = row.get("parse_status");
        let embed_status: Option<String> = row.get("embed_status");
        let pages: i64 = row.get("pages");
        let pdf_route = pdf_route_files(data_dir, &rel);
        let settled = match status.as_deref() {
            Some("quality") => {
                pages > 0 && embed_status.is_none() && !needs_conversion(data_dir, &rel)
            }
            Some("error") => !pdf_route,
            Some("skipped") => true,
            _ => false,
        };
        if settled {
            continue;
        }
        match convert(data_dir, &rel) {
            Ok(pages) => {
                record(pool, subject_id, &rel, &pages).await?;
            }
            Err(error) => {
                eprintln!("[oculus] spreadsheet {rel}: {error}");
                forget(pool, subject_id, &rel).await?;
            }
        }
        converted += 1;
    }
    Ok(converted)
}

/// App startup: [`reconcile`] on a thread of its own.
pub fn reconcile_in_background() {
    let spawned = std::thread::Builder::new()
        .name("oculus-sheets".into())
        .spawn(|| {
            let data_dir = paths::data_dir();
            let outcome = tauri::async_runtime::block_on(async {
                let pool = crate::db::store::open_pool().await?;
                let result = reconcile(&pool, &data_dir).await;
                pool.close().await;
                result
            });
            match outcome {
                Ok(0) => {}
                Ok(n) => eprintln!("[oculus] spreadsheets: converted {n} to text"),
                Err(e) => eprintln!("[oculus] spreadsheets: {e}"),
            }
        });
    if let Err(e) = spawned {
        eprintln!("[oculus] spreadsheets: {e}");
    }
}
