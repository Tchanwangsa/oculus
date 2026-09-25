//! Semantic page retrieval over page *images* (see CLAUDE.md and
//! `docs/retrieval.md`).
//!
//! `embed::backend` embeds each page -> blobs land in `pages` beside the
//! page's markdown -> a query is embedded by the same backend and ranked by
//! dot product in a brute-force scan -> hits carry markdown and (file, page).
//!
//! **One space, or the ranking is noise.** Every scan filters on
//! `pages.embed_model` and `pages.embed_dim`: a dot product across two models
//! still sorts, confidently and meaninglessly. Vectors from a retired model
//! stay in the table but are never scanned, and `IndexStats` counts them apart.

/// The lexical half of search: an FTS5 index over `pages.markdown`, for exact
/// words. Queried from the frontend (`searchPageText` in `app/src/lib/db.ts`).
///
/// External content (`content='pages'`), kept in step by triggers whichever
/// process writes. `UPDATE OF markdown`, so an embed's blob write re-indexes
/// nothing.
///
/// **A page deleted by `files`' `ON DELETE CASCADE` does not fire the delete
/// trigger** (SQLite needs `recursive_triggers`), so the index can hold dead
/// entries. Harmless: every read joins `pages ON pages.id = pages_fts.rowid`.
pub const PAGES_FTS_SQL: &str = r#"
CREATE VIRTUAL TABLE IF NOT EXISTS pages_fts USING fts5(
    markdown,
    content='pages',
    content_rowid='id',
    tokenize="unicode61 remove_diacritics 2"
);
INSERT INTO pages_fts(rowid, markdown) SELECT id, markdown FROM pages;
CREATE TRIGGER pages_fts_ai AFTER INSERT ON pages BEGIN
    INSERT INTO pages_fts(rowid, markdown) VALUES (new.id, new.markdown);
END;
CREATE TRIGGER pages_fts_ad AFTER DELETE ON pages BEGIN
    INSERT INTO pages_fts(pages_fts, rowid, markdown)
    VALUES ('delete', old.id, old.markdown);
END;
CREATE TRIGGER pages_fts_au AFTER UPDATE OF markdown ON pages BEGIN
    INSERT INTO pages_fts(pages_fts, rowid, markdown)
    VALUES ('delete', old.id, old.markdown);
    INSERT INTO pages_fts(rowid, markdown) VALUES (new.id, new.markdown);
END;
"#;

use std::path::{Path, PathBuf};
use std::sync::Arc;

use base64::Engine;
use serde::Serialize;
use sqlx::Row;

use crate::embed;
use crate::store::{db_path, pool};

/// Somewhere for a long-running embed to report to. Owned, because the embed
/// runs on a blocking thread and the callback moves there with it.
pub type ProgressSink = Arc<dyn Fn(embed::Progress) + Send + Sync>;

/// Why an ingest failed, with `EmbedError`'s discriminants intact for the
/// pipeline row. They are `None` for a failure that never reached a backend
/// (file missing, database write refused): unknown, not `false`.
#[derive(Debug, Clone)]
pub struct IngestError {
    pub message: String,
    pub kind: Option<&'static str>,
    pub retryable: Option<bool>,
    pub latching: Option<bool>,
}

impl From<embed::EmbedError> for IngestError {
    fn from(error: embed::EmbedError) -> Self {
        Self {
            message: error.to_string(),
            kind: Some(error.kind()),
            retryable: Some(error.retryable()),
            latching: Some(error.latching()),
        }
    }
}

impl From<String> for IngestError {
    fn from(message: String) -> Self {
        Self { message, kind: None, retryable: None, latching: None }
    }
}

impl std::fmt::Display for IngestError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

#[derive(Serialize)]
pub struct IngestSummary {
    pub file_id: i64,
    pub pages_embedded: usize,
    pub pages_with_markdown: usize,
    pub model: String,
    pub dim: usize,
    pub skipped: bool,
}

#[derive(Serialize)]
pub struct SearchHit {
    pub file_id: i64,
    pub page_no: i64,
    pub score: f32,
    pub filename: String,
    pub relative_path: String,
    pub subject_id: i64,
    pub markdown: String,
}

/// What the index contains, split into what can be searched **now** (the
/// current space) and what is merely stored (every blob in the table).
#[derive(Serialize)]
pub struct IndexStats {
    /// Searchable now: distinct files with at least one current-space vector.
    pub files_embedded: i64,
    /// Searchable now: pages with a current-space vector.
    pub pages_embedded: i64,
    /// Of those, how many also carry markdown to hydrate an answer from.
    pub pages_with_markdown: i64,
    /// The space the counts above are counted in.
    pub model: Option<String>,
    pub dim: Option<i64>,
    /// Every vector in the table, whichever model wrote it.
    pub files_stored: i64,
    pub pages_stored: i64,
    /// Stored but not searchable; these re-embed on the next `oculus index`.
    pub pages_stale: i64,
    /// Which models those came from, so a message can name them.
    pub stale_models: Vec<String>,
}

// ── Plumbing ─────────────────────────────────────────────────────────────────

/// The space this app searches in, off the seam's constants: `stats` must
/// answer with no API key stored. `embed::Health::check` refuses any backend
/// that disagrees, so the two agree by construction.
fn current_space() -> (&'static str, i64) {
    (embed::EMBED_MODEL, embed::EMBED_DIM as i64)
}

/// The stored blob *is* the wire string, base64-decoded. Not through floats,
/// which would re-normalise and could move the last bit.
fn blob_from_wire(page_no: u32, encoded: &str) -> Result<Vec<u8>, String> {
    base64::engine::general_purpose::STANDARD
        .decode(encoded)
        .map_err(|e| format!("page {page_no}: bad base64: {e}"))
}

// ── Ingest ───────────────────────────────────────────────────────────────────

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
/// trip, with no timeout** — the client owns its pacing (see CLAUDE.md).
pub async fn ingest_reporting(
    db_file: &Path,
    file_id: i64,
    pdf_path: String,
    force: bool,
    on_progress: ProgressSink,
) -> Result<IngestSummary, IngestError> {
    let pdf = PathBuf::from(&pdf_path);
    if !pdf.is_file() {
        return Err(format!("not on disk: {}", pdf.display()).into());
    }

    // Markdown is optional: a PDF can embed before its parse lands. The parse
    // record also gives the expected page count.
    let parsed = crate::parse::read_record(&pdf);
    let page_count = parsed.as_ref().map(|p| p.page_count).unwrap_or(0);
    let markdown: std::collections::HashMap<i64, String> = parsed
        .map(|p| p.pages.into_iter().map(|page| (page.page_no as i64, page.markdown)).collect())
        .unwrap_or_default();

    let skipped = !force && embed::is_embedded(&pdf);
    let record = if skipped {
        embed::read_record(&pdf)
            .ok_or_else(|| IngestError::from(format!("{}: embedding record vanished", pdf.display())))?
    } else {
        let target = pdf.clone();
        tauri::async_runtime::spawn_blocking(move || {
            let backend = embed::backend()?;
            embed::preflight(backend.as_ref())?;
            let output = backend.embed(&target, page_count, &|progress| on_progress(progress))?;
            // The record lands (atomically) before the database hears of it.
            output.write(&target)?;
            Ok::<_, embed::EmbedError>(output)
        })
        .await
        // A join failure is our panic, not the backend's: no discriminants.
        .map_err(|e| IngestError::from(e.to_string()))?
        .map_err(IngestError::from)?
    };

    let db = pool(db_file).await.map_err(IngestError::from)?;
    let mut tx = db.begin().await.map_err(|e| IngestError::from(e.to_string()))?;
    let mut with_md = 0usize;

    for page in &record.pages {
        let vec_bytes = blob_from_wire(page.page_no, &page.vector).map_err(IngestError::from)?;
        let page_no = page.page_no as i64;
        let md = markdown.get(&page_no).cloned().unwrap_or_default();
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
        .bind(&md)
        .bind(vec_bytes)
        .bind(&record.model)
        .bind(record.dim as i64)
        .execute(&mut *tx)
        .await
        .map_err(|e| IngestError::from(format!("upsert page {}: {e}", page.page_no)))?;
    }

    sqlx::query("UPDATE files SET embed_status = 'done', embedded_at = datetime('now') WHERE id = ?1")
        .bind(file_id)
        .execute(&mut *tx)
        .await
        .map_err(|e| IngestError::from(e.to_string()))?;

    tx.commit().await.map_err(|e| IngestError::from(e.to_string()))?;
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

// ── Search ───────────────────────────────────────────────────────────────────

/// Rank pages against a natural-language query.
///
/// `subject_id` scopes the scan to one subject; omit it to search everything.
pub async fn search(
    db_file: &Path,
    query: String,
    limit: i64,
    subject_id: Option<i64>,
) -> Result<Vec<SearchHit>, String> {
    let ids: Vec<i64> = subject_id.into_iter().collect();
    search_in(db_file, query, limit, &ids).await
}

/// `search`, but over a set of subjects (empty means every subject), so the
/// CLI's prefix codes embed the query once.
pub async fn search_in(
    db_file: &Path,
    query: String,
    limit: i64,
    subject_ids: &[i64],
) -> Result<Vec<SearchHit>, String> {
    let (qvec, model, dim) = tauri::async_runtime::spawn_blocking(move || {
        let backend = embed::backend()?;
        // Before the query costs anything; also names the space to scan.
        let health = embed::preflight(backend.as_ref())?;
        let vector = backend.embed_query(&query)?;
        Ok::<_, embed::EmbedError>((vector, health.model, health.dim))
    })
    .await
    .map_err(|e| e.to_string())?
    .map_err(|e| e.to_string())?;

    if qvec.len() != dim {
        return Err(format!("query vector is {} dims, the backend claims {dim}", qvec.len()));
    }
    rank(db_file, &qvec, &model, dim as i64, limit, subject_ids).await
}

/// The brute-force half, with the query already embedded; split out so the
/// space filter tests without a key or a network.
async fn rank(
    db_file: &Path,
    qvec: &[f32],
    model: &str,
    dim: i64,
    limit: i64,
    subject_ids: &[i64],
) -> Result<Vec<SearchHit>, String> {
    let limit = limit.clamp(1, 50) as usize;
    let db = pool(db_file).await?;
    // Inlined: sqlx has no list binding, and these are i64s.
    let filter = if subject_ids.is_empty() {
        String::new()
    } else {
        let list: Vec<String> = subject_ids.iter().map(|i| i.to_string()).collect();
        format!(" AND f.subject_id IN ({})", list.join(","))
    };
    // The space predicate, in SQL so retired vectors are never decoded.
    let sql = format!(
        r#"
        SELECT p.file_id, p.page_no, p.embedding, p.markdown,
               f.filename, f.relative_path, f.subject_id
        FROM pages p
        JOIN files f ON f.id = p.file_id
        WHERE p.embedding IS NOT NULL
          AND p.embed_model = ?1
          AND p.embed_dim   = ?2{filter}
    "#
    );
    let rows = sqlx::query(&sql)
        .bind(model)
        .bind(dim)
        .fetch_all(&db)
        .await
        .map_err(|e| e.to_string())?;
    db.close().await;

    let mut scored: Vec<SearchHit> = Vec::with_capacity(rows.len());
    for row in rows {
        let blob: Vec<u8> = row.try_get("embedding").map_err(|e| e.to_string())?;
        let v = embed::unpack_vector(&blob);
        // A row can claim the current dim and hold another width.
        if v.len() != qvec.len() {
            continue;
        }
        let score: f32 = v.iter().zip(qvec).map(|(a, b)| a * b).sum();
        scored.push(SearchHit {
            file_id: row.try_get("file_id").map_err(|e| e.to_string())?,
            page_no: row.try_get("page_no").map_err(|e| e.to_string())?,
            score,
            filename: row.try_get("filename").map_err(|e| e.to_string())?,
            relative_path: row.try_get("relative_path").map_err(|e| e.to_string())?,
            subject_id: row.try_get("subject_id").map_err(|e| e.to_string())?,
            markdown: row.try_get("markdown").map_err(|e| e.to_string())?,
        });
    }

    scored.sort_by(|a, b| b.score.total_cmp(&a.score));
    scored.truncate(limit);
    Ok(scored)
}

// ── Stats ────────────────────────────────────────────────────────────────────

/// What the index actually contains — "nothing indexed", "indexed, no
/// matches" and "indexed by a retired model" are different answers.
pub async fn stats(db_file: &Path) -> Result<IndexStats, String> {
    let (model, dim) = current_space();
    let db = pool(db_file).await?;

    // One pass, so the current-space and whole-table counts are consistent.
    let row = sqlx::query(
        r#"SELECT COUNT(DISTINCT CASE WHEN embed_model = ?1 AND embed_dim = ?2
                                      THEN file_id END)                    AS files_embedded,
                  SUM(CASE WHEN embed_model = ?1 AND embed_dim = ?2
                           THEN 1 ELSE 0 END)                              AS pages_embedded,
                  SUM(CASE WHEN embed_model = ?1 AND embed_dim = ?2
                            AND markdown != '' THEN 1 ELSE 0 END)          AS pages_with_markdown,
                  COUNT(DISTINCT file_id)                                  AS files_stored,
                  COUNT(*)                                                 AS pages_stored
           FROM pages WHERE embedding IS NOT NULL"#,
    )
    .bind(model)
    .bind(dim)
    .fetch_one(&db)
    .await
    .map_err(|e| e.to_string())?;

    // Named, not just counted, so a message can say which model.
    let stale_models: Vec<String> = sqlx::query(
        r#"SELECT DISTINCT embed_model FROM pages
           WHERE embedding IS NOT NULL
             AND NOT (embed_model IS ?1 AND embed_dim IS ?2)
             AND embed_model IS NOT NULL
           ORDER BY embed_model"#,
    )
    .bind(model)
    .bind(dim)
    .fetch_all(&db)
    .await
    .map_err(|e| e.to_string())?
    .iter()
    .filter_map(|row| row.try_get::<String, _>("embed_model").ok())
    .collect();
    db.close().await;

    let pages_embedded: i64 =
        row.try_get::<Option<i64>, _>("pages_embedded").ok().flatten().unwrap_or(0);
    let pages_stored: i64 = row.try_get("pages_stored").unwrap_or(0);

    Ok(IndexStats {
        files_embedded: row.try_get("files_embedded").unwrap_or(0),
        pages_embedded,
        pages_with_markdown: row
            .try_get::<Option<i64>, _>("pages_with_markdown")
            .ok()
            .flatten()
            .unwrap_or(0),
        model: Some(model.to_string()),
        dim: Some(dim),
        files_stored: row.try_get("files_stored").unwrap_or(0),
        pages_stored,
        pages_stale: (pages_stored - pages_embedded).max(0),
        stale_models,
    })
}

// ── Tauri commands ───────────────────────────────────────────────────────────
//
// Thin wrappers: the logic above takes a database path, not an AppHandle, so
// it runs headlessly (`src/bin/retrieval_smoke.rs`, the CLI).

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
    let base = crate::paths::data_dir();
    let pdf_rel = match crate::paths::doc_pdf_rel(&relative_path) {
        Some(rel) => rel,
        None => {
            let message = format!("{relative_path}: no PDF representation to embed");
            embed::events::failed_with(&relative_path, subject_id, message.clone(), None, None, None);
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::Scratch;

    use sqlx::sqlite::SqliteConnectOptions;
    use sqlx::SqlitePool;

    /// A retired model, as a literal the code under test cannot change.
    const RETIRED_MODEL: &str = "Qwen3-VL-Embedding-2B";

    /// Just enough schema for the scan; this tests the predicate, not the schema.
    async fn fixture(path: &Path) -> SqlitePool {
        let options = SqliteConnectOptions::new().filename(path).create_if_missing(true);
        let db = SqlitePool::connect_with(options).await.unwrap();
        sqlx::query(
            "CREATE TABLE files (
               id INTEGER PRIMARY KEY, subject_id INTEGER, filename TEXT,
               relative_path TEXT, embed_status TEXT, embedded_at TEXT)",
        )
        .execute(&db)
        .await
        .unwrap();
        sqlx::query(
            "CREATE TABLE pages (
               id INTEGER PRIMARY KEY, file_id INTEGER, page_no INTEGER,
               markdown TEXT NOT NULL DEFAULT '', embedding BLOB,
               embed_model TEXT, embed_dim INTEGER, embedded_at TEXT,
               UNIQUE(file_id, page_no))",
        )
        .execute(&db)
        .await
        .unwrap();
        db
    }

    async fn add_file(db: &SqlitePool, id: i64, subject_id: i64, name: &str) {
        sqlx::query("INSERT INTO files (id, subject_id, filename, relative_path) VALUES (?1, ?2, ?3, ?4)")
            .bind(id)
            .bind(subject_id)
            .bind(name)
            .bind(format!("courses/X/{name}"))
            .execute(db)
            .await
            .unwrap();
    }

    /// A unit vector along one axis, so every score is predictable.
    fn axis(index: usize) -> Vec<f32> {
        let mut v = vec![0.0f32; embed::EMBED_DIM];
        v[index] = 1.0;
        v
    }

    async fn add_page(
        db: &SqlitePool,
        file_id: i64,
        page_no: i64,
        vector: &[f32],
        model: &str,
        dim: i64,
    ) {
        sqlx::query(
            "INSERT INTO pages (file_id, page_no, markdown, embedding, embed_model, embed_dim)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        )
        .bind(file_id)
        .bind(page_no)
        .bind(format!("page {page_no} of {file_id}"))
        .bind(embed::pack_vector(vector).unwrap())
        .bind(model)
        .bind(dim)
        .execute(db)
        .await
        .unwrap();
    }

    /// A stale vector aimed straight at the query would win if it were scanned.
    #[tokio::test]
    async fn the_scan_never_sees_another_models_vectors() {
        let scratch = Scratch::new("retrieval-space");
        let db = fixture(&scratch.join("oculus.db")).await;
        add_file(&db, 1, 10, "current.pdf").await;
        add_file(&db, 2, 10, "retired.pdf").await;
        // Current space, a middling match.
        let mut lukewarm = axis(0);
        lukewarm[1] = 1.0;
        add_page(&db, 1, 1, &lukewarm, embed::EMBED_MODEL, embed::EMBED_DIM as i64).await;
        // Retired space, a perfect match — and it must still lose.
        add_page(&db, 2, 1, &axis(0), RETIRED_MODEL, embed::EMBED_DIM as i64).await;
        db.close().await;

        let hits = rank(&scratch.join("oculus.db"), &axis(0), embed::EMBED_MODEL, embed::EMBED_DIM as i64, 5, &[])
            .await
            .unwrap();
        assert_eq!(hits.len(), 1, "a retired model's vectors were ranked");
        assert_eq!(hits[0].file_id, 1);
    }

    /// Same model, different `embed_dim`: the pair is the key.
    #[tokio::test]
    async fn a_truncation_is_a_different_space_too() {
        let scratch = Scratch::new("retrieval-dim");
        let db = fixture(&scratch.join("oculus.db")).await;
        add_file(&db, 1, 10, "narrow.pdf").await;
        add_page(&db, 1, 1, &axis(0), embed::EMBED_MODEL, 256).await;
        db.close().await;

        let hits = rank(&scratch.join("oculus.db"), &axis(0), embed::EMBED_MODEL, embed::EMBED_DIM as i64, 5, &[])
            .await
            .unwrap();
        assert!(hits.is_empty(), "a vector of another width was ranked");
    }

    #[tokio::test]
    async fn the_subject_filter_still_applies_within_the_space() {
        let scratch = Scratch::new("retrieval-subject");
        let db = fixture(&scratch.join("oculus.db")).await;
        add_file(&db, 1, 10, "mine.pdf").await;
        add_file(&db, 2, 20, "theirs.pdf").await;
        add_page(&db, 1, 1, &axis(0), embed::EMBED_MODEL, embed::EMBED_DIM as i64).await;
        add_page(&db, 2, 1, &axis(0), embed::EMBED_MODEL, embed::EMBED_DIM as i64).await;
        db.close().await;

        let hits =
            rank(&scratch.join("oculus.db"), &axis(0), embed::EMBED_MODEL, embed::EMBED_DIM as i64, 5, &[20])
                .await
                .unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].subject_id, 20);
    }

    #[tokio::test]
    async fn stats_separate_searchable_from_merely_stored() {
        let scratch = Scratch::new("retrieval-stats");
        let db = fixture(&scratch.join("oculus.db")).await;
        add_file(&db, 1, 10, "old.pdf").await;
        add_file(&db, 2, 10, "new.pdf").await;
        for page in 1..=3 {
            add_page(&db, 1, page, &axis(0), RETIRED_MODEL, embed::EMBED_DIM as i64).await;
        }
        add_page(&db, 2, 1, &axis(0), embed::EMBED_MODEL, embed::EMBED_DIM as i64).await;
        db.close().await;

        let stats = stats(&scratch.join("oculus.db")).await.unwrap();
        assert_eq!(stats.pages_embedded, 1, "stale pages counted as searchable");
        assert_eq!(stats.files_embedded, 1);
        assert_eq!(stats.pages_stored, 4);
        assert_eq!(stats.files_stored, 2);
        assert_eq!(stats.pages_stale, 3);
        assert_eq!(stats.stale_models, vec![RETIRED_MODEL.to_string()]);
        assert_eq!(stats.model.as_deref(), Some(embed::EMBED_MODEL));
        assert_eq!(stats.dim, Some(embed::EMBED_DIM as i64));
    }

    /// Empty, but still names the space it would search.
    #[tokio::test]
    async fn an_empty_index_is_zero_everywhere() {
        let scratch = Scratch::new("retrieval-empty");
        fixture(&scratch.join("oculus.db")).await.close().await;
        let stats = stats(&scratch.join("oculus.db")).await.unwrap();
        assert_eq!(stats.pages_embedded, 0);
        assert_eq!(stats.pages_stored, 0);
        assert_eq!(stats.pages_stale, 0);
        assert!(stats.stale_models.is_empty());
        assert_eq!(stats.model.as_deref(), Some(embed::EMBED_MODEL));
    }
}

#[cfg(test)]
mod fts_tests {
    use sqlx::sqlite::SqlitePoolOptions;
    use sqlx::SqlitePool;

    use super::PAGES_FTS_SQL;

    /// A `pages` table and the FTS index over it, from the SQL migration 35 runs.
    async fn pool() -> SqlitePool {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .expect("in-memory sqlite");
        sqlx::query(
            "CREATE TABLE pages (
                 id        INTEGER PRIMARY KEY AUTOINCREMENT,
                 file_id   INTEGER NOT NULL,
                 page_no   INTEGER NOT NULL,
                 markdown  TEXT NOT NULL DEFAULT '',
                 embedding BLOB,
                 UNIQUE(file_id, page_no)
             )",
        )
        .execute(&pool)
        .await
        .expect("pages");
        // `raw_sql`: several statements, as the migration runner executes it.
        sqlx::raw_sql(PAGES_FTS_SQL)
            .execute(&pool)
            .await
            .expect("pages_fts");
        pool
    }

    async fn hits(pool: &SqlitePool, q: &str) -> Vec<i64> {
        sqlx::query_scalar(
            "SELECT p.page_no FROM pages_fts
               JOIN pages p ON p.id = pages_fts.rowid
              WHERE pages_fts MATCH ?1
              ORDER BY bm25(pages_fts)",
        )
        .bind(q)
        .fetch_all(pool)
        .await
        .expect("match")
    }

    /// Without FTS5 in the linked SQLite, migration 35 fails and the app cannot
    /// open its database; `libsqlite3-sys`' bundled build must keep it.
    #[tokio::test]
    async fn fts5_indexes_inserts_updates_and_deletes() {
        let pool = pool().await;
        sqlx::query("INSERT INTO pages (file_id, page_no, markdown) VALUES (1, 1, ?1)")
            .bind("A Nash equilibrium is a profile of strategies")
            .execute(&pool)
            .await
            .expect("insert");
        assert_eq!(hits(&pool, "nash").await, vec![1], "insert trigger");

        // An embed's blob write must not drop the row from the index.
        sqlx::query("UPDATE pages SET embedding = ?1 WHERE page_no = 1")
            .bind(vec![0u8; 8])
            .execute(&pool)
            .await
            .expect("embed");
        assert_eq!(hits(&pool, "nash").await, vec![1], "blob write left the index alone");

        // A re-parse replaces the text: the old terms must stop matching.
        sqlx::query("UPDATE pages SET markdown = ?1 WHERE page_no = 1")
            .bind("A dominant strategy dominates every alternative")
            .execute(&pool)
            .await
            .expect("reparse");
        assert!(hits(&pool, "nash").await.is_empty(), "update trigger cleared the old terms");
        assert_eq!(hits(&pool, "dominant").await, vec![1], "update trigger indexed the new ones");

        sqlx::query("DELETE FROM pages WHERE page_no = 1")
            .execute(&pool)
            .await
            .expect("delete");
        assert!(hits(&pool, "dominant").await.is_empty(), "delete trigger");
    }

    /// `snippet()` marks matched prose; prefix terms answer mid-word.
    #[tokio::test]
    async fn snippet_marks_the_matched_words() {
        let pool = pool().await;
        sqlx::query("INSERT INTO pages (file_id, page_no, markdown) VALUES (1, 1, ?1)")
            .bind("Shannon entropy measures the uncertainty of a source")
            .execute(&pool)
            .await
            .expect("insert");
        let snippet: String = sqlx::query_scalar(
            "SELECT snippet(pages_fts, 0, '<', '>', '…', 8)
               FROM pages_fts WHERE pages_fts MATCH ?1",
        )
        .bind("\"entrop\"*")
        .fetch_one(&pool)
        .await
        .expect("snippet");
        assert!(snippet.contains("<entropy>"), "got {snippet}");
    }
}
