use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::db::store::pool;
use crate::embed;

use super::plumbing::blob_from_wire;
use super::{IngestError, IngestSummary, ProgressSink};

/// Embed one PDF's pages and store them against `file_id`. Idempotent: a PDF
/// whose `.emb.json` is current (`embed::is_embedded`) is not re-embedded, but
/// its record is still folded into `pages`, since an artifact on disk is no
/// promise the database has it.
pub async fn ingest(
    db_file: &Path,
    file_id: i64,
    pdf_path: String,
    force: bool,
) -> Result<IngestSummary, String> {
    ingest_reporting(db_file, file_id, pdf_path, force, Arc::new(|_| {}))
        .await
        .map_err(|e| e.message)
}

/// `ingest`, plus a progress callback. **Blocks for the whole cloud round
/// trip, with no timeout** — the client owns its pacing (see `docs/retrieval.md`).
pub async fn ingest_reporting(
    db_file: &Path,
    file_id: i64,
    pdf_path: String,
    force: bool,
    on_progress: ProgressSink,
) -> Result<IngestSummary, IngestError> {
    // Artifact JSON can be large; keep its reads and decoding on the same
    // blocking worker as the backend, including already-embedded files.
    let (record, markdown, skipped) = tauri::async_runtime::spawn_blocking(move || {
        let pdf = PathBuf::from(pdf_path);
        if !pdf.is_file() {
            return Err(IngestError::from(format!("not on disk: {}", pdf.display())));
        }
        let parsed = crate::parse::read_record(&pdf);
        let expected_pages = parsed.as_ref().map(|record| record.page_count);
        let page_count = expected_pages.unwrap_or(0);
        let markdown: HashMap<i64, String> = parsed
            .map(|p| {
                p.pages
                    .into_iter()
                    .map(|page| (page.page_no as i64, page.markdown))
                    .collect()
            })
            .unwrap_or_default();
        let cached = if force {
            None
        } else {
            embed::read_record(&pdf).filter(|record| record.is_current(expected_pages))
        };
        let skipped = cached.is_some();
        let record = if let Some(record) = cached {
            record
        } else {
            let backend = embed::backend()?;
            embed::preflight(backend.as_ref())?;
            let output = backend.embed(&pdf, page_count, &|progress| on_progress(progress))?;
            // The record lands (atomically) before the database hears of it.
            output.write(&pdf)?;
            output
        };
        Ok::<_, IngestError>((record, markdown, skipped))
    })
    .await
    // A join failure is our panic, not the backend's: no discriminants.
    .map_err(|e| IngestError::from(e.to_string()))??;

    let db = pool(db_file).await.map_err(IngestError::from)?;
    let mut tx = db
        .begin()
        .await
        .map_err(|e| IngestError::from(e.to_string()))?;
    let mut with_md = 0usize;

    for page in &record.pages {
        let vec_bytes = blob_from_wire(page.page_no, &page.vector).map_err(IngestError::from)?;
        let page_no = page.page_no as i64;
        let md = markdown
            .get(&page_no)
            .map(String::as_str)
            .unwrap_or_default();
        if !md.is_empty() {
            with_md += 1;
        }

        // Keep existing markdown if the parse has not landed. `embed_model` /
        // `embed_dim` come off the record: the row must say which space its
        // bytes are actually in.
        sqlx::query(
            r#"INSERT INTO pages (file_id, page_no, markdown, embedding, embed_model, embed_dim, embedded_at)
               VALUES (?1, ?2, ?3, ?4, ?5, ?6, datetime('now'))
               ON CONFLICT(file_id, page_no) DO UPDATE SET
                 markdown    = CASE WHEN excluded.markdown != '' THEN excluded.markdown ELSE pages.markdown END,
                 embedding   = excluded.embedding,
                 embed_model = excluded.embed_model,
                 embed_dim   = excluded.embed_dim,
                 embedded_at = excluded.embedded_at"#,
        )
        .bind(file_id)
        .bind(page_no)
        .bind(md)
        .bind(vec_bytes)
        .bind(&record.model)
        .bind(record.dim as i64)
        .execute(&mut *tx)
        .await
        .map_err(|e| IngestError::from(format!("upsert page {}: {e}", page.page_no)))?;
    }

    sqlx::query(
        "UPDATE files SET embed_status = 'done', embedded_at = datetime('now') WHERE id = ?1",
    )
    .bind(file_id)
    .execute(&mut *tx)
    .await
    .map_err(|e| IngestError::from(e.to_string()))?;

    tx.commit()
        .await
        .map_err(|e| IngestError::from(e.to_string()))?;
    db.close().await;

    Ok(IngestSummary {
        file_id,
        pages_embedded: record.pages.len(),
        pages_with_markdown: with_md,
        model: record.model,
        dim: record.dim,
        skipped,
    })
}
