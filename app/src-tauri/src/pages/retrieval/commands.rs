//! Thin wrappers: the logic in the sibling files takes a database path, not an
//! `AppHandle`, so it runs headlessly in the CLI.

use std::sync::Arc;

use crate::db::store::db_path;
use crate::embed;

use super::{ingest_reporting, search, stats, IndexStats, IngestSummary, SearchHit};

/// `relative_path` is relative to the app data dir; the frontend never
/// handles absolute paths. Narrates itself over `embed-status`, keyed on
/// `(subject_id, relative_path)` like the parse row — emitted here rather than
/// in `ingest`, which the CLI also runs.
#[tauri::command]
pub async fn embed_file(
    file_id: i64,
    subject_id: i64,
    relative_path: String,
    force: Option<bool>,
) -> Result<IngestSummary, String> {
    let base = crate::library::paths::data_dir();
    let pdf_rel = match crate::library::paths::doc_pdf_rel(&relative_path) {
        Some(rel) => rel,
        None => {
            let message = format!("{relative_path}: no PDF representation to embed");
            embed::events::failed_with(
                &relative_path,
                subject_id,
                message.clone(),
                None,
                None,
                None,
            );
            return Err(message);
        }
    };
    let pdf = base.join(&pdf_rel);
    let db = db_path();

    embed::events::queued(&relative_path, subject_id);

    let path = relative_path.clone();
    let outcome = ingest_reporting(
        &db,
        file_id,
        pdf.to_string_lossy().to_string(),
        force.unwrap_or(false),
        Arc::new(move |progress: embed::Progress| {
            embed::events::running(&path, subject_id, progress);
        }),
    )
    .await;

    match outcome {
        Ok(summary) => {
            embed::events::embedded(&relative_path, subject_id, summary.pages_embedded as u32);
            Ok(summary)
        }
        Err(error) => {
            // Discriminants travel on the event; the caller gets the sentence.
            embed::events::failed_with(
                &relative_path,
                subject_id,
                error.message.clone(),
                error.kind,
                error.retryable,
                error.latching,
            );
            Err(error.message)
        }
    }
}

#[tauri::command]
pub async fn search_pages(
    query: String,
    limit: Option<i64>,
    subject_id: Option<i64>,
) -> Result<Vec<SearchHit>, String> {
    let db = db_path();
    search(&db, query, limit.unwrap_or(5), subject_id).await
}

#[tauri::command]
pub async fn embedding_stats() -> Result<IndexStats, String> {
    let db = db_path();
    stats(&db).await
}
