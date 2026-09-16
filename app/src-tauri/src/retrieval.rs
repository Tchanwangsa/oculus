//! Semantic page retrieval.
//!
//! The index is built from page *images*, not scraped text — measured on the
//! MULT20015 decks, image embeddings roughly double recall on formula slides,
//! screenshots and diagrams, where text extraction returns things like
//! `56 = 7(( 7() 7)(`. That constraint is in `CLAUDE.md` and it binds the
//! cloud path exactly as it bound the local one: `embed/raster.rs` rasterises
//! the page and the pixels are what get embedded.
//!
//! Flow: the seam's backend (`embed::backend`) rasterises and embeds each page
//! -> blobs land in `pages` alongside that page's markdown -> a query is
//! embedded by the *same* backend and ranked by dot product -> hits carry the
//! markdown for the answer and (file, page) for the deep link. Nothing
//! downstream of ranking touches a vector.
//!
//! This is deliberately a brute-force scan. A degree of coursework is a few
//! thousand pages; at 512 dims that is single-digit MB and a few milliseconds,
//! so an index would be machinery without a payoff.
//!
//! **One space, or the ranking is noise.** Every scan below filters on
//! `pages.embed_model` and `pages.embed_dim` against the space the seam is
//! currently embedding into. This is not tidiness. A dot product between a
//! Voyage vector and a Qwen one is not a worse score, it is a meaningless one,
//! and it still sorts — a search that returns confident, well-formatted,
//! unrelated slides with nothing anywhere looking broken. The columns exist
//! for this; `embed::Health::check` refuses a backend that disagrees about the
//! space for the same reason. Vectors from a retired model stay in the table
//! (they are free, and dropping them is `embed_set_engine`'s job) but they are
//! never scanned, and `IndexStats` reports them separately so "2,980 pages
//! indexed" can never mean "2,980 pages searchable".

use std::path::{Path, PathBuf};
use std::sync::Arc;

use base64::Engine;
use serde::Serialize;
use sqlx::Row;
use tauri::{AppHandle, Manager};

use crate::embed;

/// Somewhere for a long-running embed to report to.
///
/// Owned rather than borrowed (`&dyn Fn`, which is what the seam takes)
/// because the embedding itself runs on a blocking thread and the callback has
/// to move there with it. The app has no listener for this today — there is no
/// embed stage in the pipeline table, and nothing in `useBackendEvents.ts`
/// subscribes to an embed event — so the only real consumer is the CLI's
/// in-place progress line. That is the honest shape: a cloud embed on the free
/// programme can take hours, and a silent terminal looks like a hang.
pub type ProgressSink = Arc<dyn Fn(embed::Progress) + Send + Sync>;

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

/// What the index contains, split into what can be searched **now** and what
/// is merely stored.
///
/// The split is the whole point of the type. `files_embedded` /
/// `pages_embedded` count only vectors in the current space, because those are
/// the only ones a query can be compared against; `pages_stored` counts every
/// blob in the table. When a library has been embedded by a model the app no
/// longer uses, the first number is zero and the second is not, and a UI that
/// only had one of them would say something false either way.
#[derive(Serialize)]
pub struct IndexStats {
    /// Searchable now: distinct files with at least one current-space vector.
    pub files_embedded: i64,
    /// Searchable now: pages with a current-space vector.
    pub pages_embedded: i64,
    /// Of those, how many also carry markdown to hydrate an answer from.
    pub pages_with_markdown: i64,
    /// The space the counts above are counted in — the seam's model and width,
    /// not whatever happens to be in the table.
    pub model: Option<String>,
    pub dim: Option<i64>,
    /// Every vector in the table, whichever model wrote it.
    pub files_stored: i64,
    pub pages_stored: i64,
    /// Stored but not searchable: a different model or a different width.
    /// These re-embed on the next `oculus index`; nothing migrates them.
    pub pages_stale: i64,
    /// Which models those came from, so a message can name them.
    pub stale_models: Vec<String>,
}

// ── Plumbing ─────────────────────────────────────────────────────────────────

// The pool helpers this module used to own now live in `store.rs`, which the
// parse path needs too; re-exported here only so the call sites below read
// unchanged.
use crate::store::{db_path, pool};

/// The space this app searches in.
///
/// Read off the seam's constants rather than a backend, because `stats` has to
/// answer on an install with no API key — the settings page shows the index
/// *before* a key is stored, and constructing a cloud client would fail there.
/// That is not a shortcut around `Health`: `embed::Health::check` refuses any
/// backend whose model or dim differs from these two, so a backend that is
/// usable at all agrees with them by construction. `search_in` still takes its
/// pair from the preflighted health, which is the same answer arrived at the
/// stricter way.
fn current_space() -> (&'static str, i64) {
    (embed::EMBED_MODEL, embed::EMBED_DIM as i64)
}

/// The stored blob *is* the wire string, base64-decoded: little-endian f16,
/// `EMBED_DIM` wide. Decoding straight to bytes rather than through
/// `embed::decode_vector` keeps the record on disk and the column byte-equal —
/// a round trip through floats would re-normalise and could move the last bit.
fn blob_from_wire(page_no: u32, encoded: &str) -> Result<Vec<u8>, String> {
    base64::engine::general_purpose::STANDARD
        .decode(encoded)
        .map_err(|e| format!("page {page_no}: bad base64: {e}"))
}

// ── Ingest ───────────────────────────────────────────────────────────────────

/// Embed one PDF's pages and store them against `file_id`.
///
/// Idempotent, and the two halves of that are separate on purpose. A PDF whose
/// `.emb.json` is already in this app's space is not re-embedded — but its
/// record is still folded into `pages`, because an artifact on disk is no
/// promise the database can see it (the same reasoning as `backfill_pages` on
/// the parse path). So a re-run costs a file read and an upsert, not a cloud
/// round trip.
///
/// **A record from a retired model does not count as embedded.**
/// `embed::is_embedded` compares model, dim and instruction and also checks
/// page coverage against the parse record, so the 166 files the Python
/// embedder wrote all read as needing work and re-embed here without a
/// migration script.
pub async fn ingest(
    db_file: &Path,
    file_id: i64,
    pdf_path: String,
    force: bool,
) -> Result<IngestSummary, String> {
    ingest_reporting(db_file, file_id, pdf_path, force, Arc::new(|_| {})).await
}

/// `ingest`, plus a callback for a caller that renders its own progress.
///
/// **This blocks for the whole cloud round trip**, which on an account with no
/// payment method on file is measured in hours for a large deck: 10K tokens a
/// minute against ~3,571 tokens for a 200-DPI page is under three pages a
/// minute, and the backend spends most of its wall clock inside a 429 it is
/// correctly waiting out. There is deliberately no timeout here — the client
/// owns its own deadlines, and a second limit that disagreed would silently
/// abandon work the first one was still doing. Same rule as `sync::parse_pdf`.
pub async fn ingest_reporting(
    db_file: &Path,
    file_id: i64,
    pdf_path: String,
    force: bool,
    on_progress: ProgressSink,
) -> Result<IngestSummary, String> {
    let pdf = PathBuf::from(&pdf_path);
    if !pdf.is_file() {
        return Err(format!("not on disk: {}", pdf.display()));
    }

    // Markdown is optional here: a PDF can be embedded before the parse has
    // landed, and the text gets filled in on the next run. The record is also
    // where the expected page count comes from — it is the same `page_no`
    // space the join below rests on.
    let parsed = crate::parse::read_record(&pdf);
    let page_count = parsed.as_ref().map(|p| p.page_count).unwrap_or(0);
    let markdown: std::collections::HashMap<i64, String> = parsed
        .map(|p| p.pages.into_iter().map(|page| (page.page_no as i64, page.markdown)).collect())
        .unwrap_or_default();

    let skipped = !force && embed::is_embedded(&pdf);
    let record = if skipped {
        embed::read_record(&pdf)
            .ok_or_else(|| format!("{}: embedding record vanished", pdf.display()))?
    } else {
        let target = pdf.clone();
        tauri::async_runtime::spawn_blocking(move || {
            // Never a concrete client at a call site: which backend this is, is
            // a setting. `preflight` is what refuses one that embeds into a
            // different space than the table holds.
            let backend = embed::backend()?;
            embed::preflight(backend.as_ref())?;
            let output = backend.embed(&target, page_count, &|progress| on_progress(progress))?;
            // The record lands before the database hears about it: it is the
            // only evidence the embedding finished, and it is written
            // atomically for exactly that reason.
            output.write(&target)?;
            Ok::<_, embed::EmbedError>(output)
        })
        .await
        .map_err(|e| e.to_string())?
        .map_err(|e| e.to_string())?
    };

    let db = pool(db_file).await?;
    let mut tx = db.begin().await.map_err(|e| e.to_string())?;
    let mut with_md = 0usize;

    for page in &record.pages {
        let vec_bytes = blob_from_wire(page.page_no, &page.vector)?;
        let page_no = page.page_no as i64;
        let md = markdown.get(&page_no).cloned().unwrap_or_default();
        if !md.is_empty() {
            with_md += 1;
        }

        // COALESCE on markdown so a re-embed before the parse lands does not
        // wipe text we already have. `embed_model` / `embed_dim` come off the
        // record rather than off the seam's constants: the row must say which
        // space its bytes are actually in, because that is what the scan
        // filters on.
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
        .map_err(|e| format!("upsert page {}: {e}", page.page_no))?;
    }

    sqlx::query("UPDATE files SET embed_status = 'done', embedded_at = datetime('now') WHERE id = ?1")
        .bind(file_id)
        .execute(&mut *tx)
        .await
        .map_err(|e| e.to_string())?;

    tx.commit().await.map_err(|e| e.to_string())?;
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

/// `search`, but over a set of subjects. An empty set means every subject.
///
/// Several ids matter for the CLI, which takes prefix codes: `MULT20015`
/// legitimately matches the same subject in two terms, and ranking each course
/// separately then merging would embed the query once per course.
pub async fn search_in(
    db_file: &Path,
    query: String,
    limit: i64,
    subject_ids: &[i64],
) -> Result<Vec<SearchHit>, String> {
    let (qvec, model, dim) = tauri::async_runtime::spawn_blocking(move || {
        let backend = embed::backend()?;
        // The handshake, before the query costs anything. What it returns is
        // also the space the scan is allowed to look at — asking the backend
        // rather than assuming means a query is never compared against vectors
        // some other backend wrote.
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

/// The brute-force half, with the query already embedded.
///
/// Split out so the ranking — including the space filter, which is the part
/// that can be silently wrong — is exercisable without a key, a network or a
/// rendered page.
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
    // Inlined rather than bound: sqlx has no list binding, and these are i64s
    // that came out of this same database, so there is nothing to escape.
    let filter = if subject_ids.is_empty() {
        String::new()
    } else {
        let list: Vec<String> = subject_ids.iter().map(|i| i.to_string()).collect();
        format!(" AND f.subject_id IN ({})", list.join(","))
    };
    // The model/dim predicate is the one that keeps the ranking meaningful.
    // It is bound, not inlined, and it is in SQL rather than in the loop below
    // so a library that is mostly a retired space does not pay to decode it.
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
        // Second lock on the same door: a row can claim the current dim and
        // hold a blob of another width if something wrote the two apart.
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

/// What the index actually contains — used by the UI to tell "nothing indexed"
/// apart from "indexed, no matches", and now also apart from "indexed by a
/// model this app no longer speaks".
pub async fn stats(db_file: &Path) -> Result<IndexStats, String> {
    let (model, dim) = current_space();
    let db = pool(db_file).await?;

    // One pass, two questions: the current space and the whole table. Doing it
    // as conditional aggregates rather than two scans keeps them consistent
    // with each other even if something writes between them.
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

    // Named, not just counted: "2,980 pages were embedded by
    // Qwen3-VL-Embedding-2B" is actionable where "2,980 stale pages" is not.
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
// Thin wrappers. The logic above takes a database path rather than an
// AppHandle so it can be exercised headlessly — see `src/bin/retrieval_smoke.rs`.

/// `relative_path` is relative to the app data dir, matching `read_course_file`
/// and `open_course_file` — the frontend never handles absolute paths.
#[tauri::command]
pub async fn embed_file(
    app: AppHandle,
    file_id: i64,
    relative_path: String,
    force: Option<bool>,
) -> Result<IngestSummary, String> {
    let base = app.path().app_data_dir().map_err(|e| e.to_string())?;
    let pdf_rel = crate::paths::doc_pdf_rel(&relative_path)
        .ok_or_else(|| format!("{relative_path}: no PDF representation to embed"))?;
    let pdf = base.join(&pdf_rel);
    let db = db_path(&app)?;
    ingest(&db, file_id, pdf.to_string_lossy().to_string(), force.unwrap_or(false)).await
}

#[tauri::command]
pub async fn search_pages(
    app: AppHandle,
    query: String,
    limit: Option<i64>,
    subject_id: Option<i64>,
) -> Result<Vec<SearchHit>, String> {
    let db = db_path(&app)?;
    search(&db, query, limit.unwrap_or(5), subject_id).await
}

#[tauri::command]
pub async fn embedding_stats(app: AppHandle) -> Result<IndexStats, String> {
    let db = db_path(&app)?;
    stats(&db).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    use sqlx::sqlite::SqliteConnectOptions;
    use sqlx::SqlitePool;

    /// The model every stored vector in the library was written by, until this
    /// port. Hardcoded *here* on purpose: these tests are about the app
    /// refusing to rank against it, so the name has to be a literal rather
    /// than something the code under test could quietly change.
    const RETIRED_MODEL: &str = "Qwen3-VL-Embedding-2B";

    struct Scratch {
        root: PathBuf,
    }

    impl Scratch {
        fn new(name: &str) -> Self {
            let stamp = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or_default();
            let root = std::env::temp_dir().join(format!("oculus-retrieval-{name}-{stamp}"));
            std::fs::create_dir_all(&root).unwrap();
            Self { root }
        }

        fn db(&self) -> PathBuf {
            self.root.join("oculus.db")
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            std::fs::remove_dir_all(&self.root).ok();
        }
    }

    /// Just enough of the schema for the scan: the two tables it joins, with
    /// the columns it reads. Deliberately not the app's migration list — this
    /// is testing the predicate, not the schema.
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

    /// A unit vector pointing along one axis, so a dot product against it is
    /// exactly that component and every score in these tests is predictable.
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

    /// The whole correctness argument of this port, as a test.
    ///
    /// A stale vector aimed *straight at* the query is the adversarial case:
    /// it would win every ranking if it were scanned, and nothing about the
    /// result would look wrong.
    #[tokio::test]
    async fn the_scan_never_sees_another_models_vectors() {
        let scratch = Scratch::new("space");
        let db = fixture(&scratch.db()).await;
        add_file(&db, 1, 10, "current.pdf").await;
        add_file(&db, 2, 10, "retired.pdf").await;
        // Current space, a middling match.
        let mut lukewarm = axis(0);
        lukewarm[1] = 1.0;
        add_page(&db, 1, 1, &lukewarm, embed::EMBED_MODEL, embed::EMBED_DIM as i64).await;
        // Retired space, a perfect match — and it must still lose.
        add_page(&db, 2, 1, &axis(0), RETIRED_MODEL, embed::EMBED_DIM as i64).await;
        db.close().await;

        let hits = rank(&scratch.db(), &axis(0), embed::EMBED_MODEL, embed::EMBED_DIM as i64, 5, &[])
            .await
            .unwrap();
        assert_eq!(hits.len(), 1, "a retired model's vectors were ranked");
        assert_eq!(hits[0].file_id, 1);
    }

    /// Same width, same model name, different *dim* column: the pair is the
    /// key, not either half.
    #[tokio::test]
    async fn a_truncation_is_a_different_space_too() {
        let scratch = Scratch::new("dim");
        let db = fixture(&scratch.db()).await;
        add_file(&db, 1, 10, "narrow.pdf").await;
        add_page(&db, 1, 1, &axis(0), embed::EMBED_MODEL, 256).await;
        db.close().await;

        let hits = rank(&scratch.db(), &axis(0), embed::EMBED_MODEL, embed::EMBED_DIM as i64, 5, &[])
            .await
            .unwrap();
        assert!(hits.is_empty(), "a vector of another width was ranked");
    }

    #[tokio::test]
    async fn the_subject_filter_still_applies_within_the_space() {
        let scratch = Scratch::new("subject");
        let db = fixture(&scratch.db()).await;
        add_file(&db, 1, 10, "mine.pdf").await;
        add_file(&db, 2, 20, "theirs.pdf").await;
        add_page(&db, 1, 1, &axis(0), embed::EMBED_MODEL, embed::EMBED_DIM as i64).await;
        add_page(&db, 2, 1, &axis(0), embed::EMBED_MODEL, embed::EMBED_DIM as i64).await;
        db.close().await;

        let hits =
            rank(&scratch.db(), &axis(0), embed::EMBED_MODEL, embed::EMBED_DIM as i64, 5, &[20])
                .await
                .unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].subject_id, 20);
    }

    /// `IndexStats` has to be able to say "nothing is searchable" about a
    /// table with thousands of rows in it, which is exactly the state this
    /// port leaves the real library in.
    #[tokio::test]
    async fn stats_separate_searchable_from_merely_stored() {
        let scratch = Scratch::new("stats");
        let db = fixture(&scratch.db()).await;
        add_file(&db, 1, 10, "old.pdf").await;
        add_file(&db, 2, 10, "new.pdf").await;
        for page in 1..=3 {
            add_page(&db, 1, page, &axis(0), RETIRED_MODEL, embed::EMBED_DIM as i64).await;
        }
        add_page(&db, 2, 1, &axis(0), embed::EMBED_MODEL, embed::EMBED_DIM as i64).await;
        db.close().await;

        let stats = stats(&scratch.db()).await.unwrap();
        assert_eq!(stats.pages_embedded, 1, "stale pages counted as searchable");
        assert_eq!(stats.files_embedded, 1);
        assert_eq!(stats.pages_stored, 4);
        assert_eq!(stats.files_stored, 2);
        assert_eq!(stats.pages_stale, 3);
        assert_eq!(stats.stale_models, vec![RETIRED_MODEL.to_string()]);
        assert_eq!(stats.model.as_deref(), Some(embed::EMBED_MODEL));
        assert_eq!(stats.dim, Some(embed::EMBED_DIM as i64));
    }

    /// An empty table must not report the current model as if it had rows, but
    /// it must still name the space it would search — the settings page draws
    /// that line before anything is indexed.
    #[tokio::test]
    async fn an_empty_index_is_zero_everywhere() {
        let scratch = Scratch::new("empty");
        fixture(&scratch.db()).await.close().await;
        let stats = stats(&scratch.db()).await.unwrap();
        assert_eq!(stats.pages_embedded, 0);
        assert_eq!(stats.pages_stored, 0);
        assert_eq!(stats.pages_stale, 0);
        assert!(stats.stale_models.is_empty());
        assert_eq!(stats.model.as_deref(), Some(embed::EMBED_MODEL));
    }
}
