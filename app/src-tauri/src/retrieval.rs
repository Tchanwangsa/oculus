//! Semantic page retrieval.
//!
//! The index is built from page *images*, not scraped text — measured on the
//! MULT20015 decks, image embeddings roughly double recall on formula slides,
//! screenshots and diagrams, where text extraction returns things like
//! `56 = 7(( 7() 7)(`. See `sidecar/embedder.py`.
//!
//! Flow: sidecar embeds page PNGs -> blobs land in `pages` alongside that
//! page's markdown -> a query is embedded by the same model and ranked by dot
//! product -> hits carry the markdown for the answer and (file, page) for the
//! deep link. Nothing downstream of ranking touches a vector.
//!
//! This is deliberately a brute-force scan. A degree of coursework is a few
//! thousand pages; at 512 dims that is single-digit MB and a few milliseconds,
//! so an index would be machinery without a payoff.

use std::path::{Path, PathBuf};
use std::time::Duration;

use base64::Engine;
use half::f16;
use serde::{Deserialize, Serialize};
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::{Row, SqlitePool};
use tauri::{AppHandle, Manager};

use crate::sidecar::SIDECAR_PORT;

/// Long enough for a big deck: embedding runs ~0.5s/page and a 200-page
/// reading pack is not unusual.
const EMBED_TIMEOUT: Duration = Duration::from_secs(30 * 60);
const QUERY_TIMEOUT: Duration = Duration::from_secs(60);

// ── Wire types ───────────────────────────────────────────────────────────────

#[derive(Deserialize)]
struct EmbedResponse {
    status: String,
    #[serde(default)]
    embeddings_path: Option<String>,
}

#[derive(Deserialize)]
struct QueryResponse {
    vector: String,
    dim: usize,
}

#[derive(Deserialize)]
struct EmbFile {
    model: String,
    dim: usize,
    pages: Vec<EmbPage>,
}

#[derive(Deserialize)]
struct EmbPage {
    page_no: i64,
    vector: String,
}

#[derive(Deserialize)]
struct PagesFile {
    pages: Vec<PageMd>,
}

#[derive(Deserialize)]
struct PageMd {
    page_no: i64,
    markdown: String,
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

#[derive(Serialize)]
pub struct IndexStats {
    pub files_embedded: i64,
    pub pages_embedded: i64,
    pub pages_with_markdown: i64,
    pub model: Option<String>,
    pub dim: Option<i64>,
}

// ── Plumbing ─────────────────────────────────────────────────────────────────

fn db_path(app: &AppHandle) -> Result<PathBuf, String> {
    let dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
    Ok(dir.join("oculus.db"))
}

/// Our own pool over the file tauri-plugin-sql already manages. WAL means a
/// second reader is harmless, and our writes are occasional (once per file
/// embedded), so a busy timeout is enough to stay out of the plugin's way.
pub async fn pool(path: &Path) -> Result<SqlitePool, String> {
    let opts = SqliteConnectOptions::new()
        .filename(path)
        .create_if_missing(false)
        .busy_timeout(Duration::from_secs(15));
    SqlitePoolOptions::new()
        .max_connections(2)
        .connect_with(opts)
        .await
        .map_err(|e| format!("open {}: {e}", path.display()))
}

fn decode_vector(b64: &str) -> Result<Vec<f32>, String> {
    let raw = base64::engine::general_purpose::STANDARD
        .decode(b64)
        .map_err(|e| format!("bad base64: {e}"))?;
    Ok(bytes_to_f32(&raw))
}

/// float16 little-endian -> f32. Vectors are stored normalised, so callers can
/// treat the dot product as cosine similarity.
fn bytes_to_f32(raw: &[u8]) -> Vec<f32> {
    raw.chunks_exact(2)
        .map(|c| f16::from_le_bytes([c[0], c[1]]).to_f32())
        .collect()
}

fn sidecar_url(path: &str) -> String {
    format!("http://127.0.0.1:{SIDECAR_PORT}{path}")
}

/// POST JSON to the sidecar and deserialise the reply.
///
/// `ureq`'s own json helpers are behind a feature this crate does not enable,
/// so we serialise by hand the way `ipc.rs` and `lectures.rs` already do.
fn post_json<T: for<'de> Deserialize<'de>>(
    path: &str,
    body: serde_json::Value,
    timeout: Duration,
) -> Result<T, String> {
    let text = ureq::post(&sidecar_url(path))
        .timeout(timeout)
        .set("Content-Type", "application/json")
        .send_string(&body.to_string())
        .map_err(|e| format!("sidecar {path}: {e}"))?
        .into_string()
        .map_err(|e| format!("sidecar {path}: unreadable response: {e}"))?;
    serde_json::from_str(&text).map_err(|e| format!("sidecar {path}: bad response: {e}"))
}

fn sibling(pdf_path: &str, suffix: &str) -> PathBuf {
    let p = Path::new(pdf_path);
    let stem = p.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
    p.parent().unwrap_or(Path::new(".")).join(format!("{stem}{suffix}"))
}

// ── Commands ─────────────────────────────────────────────────────────────────

/// Embed one PDF's pages and store them against `file_id`.
///
/// Idempotent: the sidecar skips a PDF whose sidecar file already matches the
/// current model and dimension, and the upsert makes a re-run a no-op.
pub async fn ingest(
    db_file: &Path,
    file_id: i64,
    pdf_path: String,
    force: bool,
) -> Result<IngestSummary, String> {
    let path_for_call = pdf_path.clone();

    let resp: EmbedResponse = tauri::async_runtime::spawn_blocking(move || {
        post_json(
            "/embed-pdf",
            serde_json::json!({ "pdf_path": path_for_call, "force": force }),
            EMBED_TIMEOUT,
        )
    })
    .await
    .map_err(|e| e.to_string())??;

    let skipped = resp.status == "skip";
    let emb_path = resp
        .embeddings_path
        .map(PathBuf::from)
        .unwrap_or_else(|| sibling(&pdf_path, ".emb.json"));

    let emb: EmbFile = serde_json::from_str(
        &std::fs::read_to_string(&emb_path)
            .map_err(|e| format!("read {}: {e}", emb_path.display()))?,
    )
    .map_err(|e| format!("parse {}: {e}", emb_path.display()))?;

    // Markdown is optional here: a PDF can be embedded before docling has
    // finished its quality pass, and the text gets filled in on the next run.
    let md_path = sibling(&pdf_path, ".pages.json");
    let markdown: std::collections::HashMap<i64, String> = std::fs::read_to_string(&md_path)
        .ok()
        .and_then(|s| serde_json::from_str::<PagesFile>(&s).ok())
        .map(|f| f.pages.into_iter().map(|p| (p.page_no, p.markdown)).collect())
        .unwrap_or_default();

    let db = pool(db_file).await?;
    let mut tx = db.begin().await.map_err(|e| e.to_string())?;
    let mut with_md = 0usize;

    for page in &emb.pages {
        let vec_bytes = base64::engine::general_purpose::STANDARD
            .decode(&page.vector)
            .map_err(|e| format!("page {}: bad base64: {e}", page.page_no))?;
        let md = markdown.get(&page.page_no).cloned().unwrap_or_default();
        if !md.is_empty() {
            with_md += 1;
        }

        // COALESCE on markdown so a re-embed before the quality parse lands
        // does not wipe text we already have.
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
        .bind(page.page_no)
        .bind(&md)
        .bind(vec_bytes)
        .bind(&emb.model)
        .bind(emb.dim as i64)
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
        pages_embedded: emb.pages.len(),
        pages_with_markdown: with_md,
        model: emb.model,
        dim: emb.dim,
        skipped,
    })
}

/// Rank pages against a natural-language query.
///
/// `subject_id` scopes the scan to one subject; omit it to search everything.
pub async fn search(
    db_file: &Path,
    query: String,
    limit: i64,
    subject_id: Option<i64>,
) -> Result<Vec<SearchHit>, String> {
    let limit = limit.clamp(1, 50) as usize;
    let q = query.clone();

    let resp: QueryResponse = tauri::async_runtime::spawn_blocking(move || {
        post_json("/embed-query", serde_json::json!({ "text": q }), QUERY_TIMEOUT)
    })
    .await
    .map_err(|e| e.to_string())??;

    let qvec = decode_vector(&resp.vector)?;
    if qvec.len() != resp.dim {
        return Err(format!("query vector is {} dims, sidecar claims {}", qvec.len(), resp.dim));
    }

    let db = pool(db_file).await?;
    let sql = r#"
        SELECT p.file_id, p.page_no, p.embedding, p.markdown,
               f.filename, f.relative_path, f.subject_id
        FROM pages p
        JOIN files f ON f.id = p.file_id
        WHERE p.embedding IS NOT NULL
          AND (?1 IS NULL OR f.subject_id = ?1)
    "#;
    let rows = sqlx::query(sql)
        .bind(subject_id)
        .fetch_all(&db)
        .await
        .map_err(|e| e.to_string())?;
    db.close().await;

    let mut scored: Vec<SearchHit> = Vec::with_capacity(rows.len());
    for row in rows {
        let blob: Vec<u8> = row.try_get("embedding").map_err(|e| e.to_string())?;
        let v = bytes_to_f32(&blob);
        // Vectors from a different model or truncation are not comparable —
        // skip rather than return a meaningless score.
        if v.len() != qvec.len() {
            continue;
        }
        let score: f32 = v.iter().zip(&qvec).map(|(a, b)| a * b).sum();
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

/// What the index actually contains — used by the UI to tell "nothing indexed"
/// apart from "indexed, no matches".
pub async fn stats(db_file: &Path) -> Result<IndexStats, String> {
    let db = pool(db_file).await?;
    let row = sqlx::query(
        r#"SELECT COUNT(DISTINCT file_id)                             AS files_embedded,
                  COUNT(*)                                            AS pages_embedded,
                  SUM(CASE WHEN markdown != '' THEN 1 ELSE 0 END)     AS pages_with_markdown,
                  MAX(embed_model)                                    AS model,
                  MAX(embed_dim)                                      AS dim
           FROM pages WHERE embedding IS NOT NULL"#,
    )
    .fetch_one(&db)
    .await
    .map_err(|e| e.to_string())?;
    db.close().await;

    Ok(IndexStats {
        files_embedded: row.try_get("files_embedded").unwrap_or(0),
        pages_embedded: row.try_get("pages_embedded").unwrap_or(0),
        pages_with_markdown: row.try_get::<Option<i64>, _>("pages_with_markdown").ok().flatten().unwrap_or(0),
        model: row.try_get("model").ok(),
        dim: row.try_get("dim").ok(),
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
    let pdf = base.join(&relative_path);
    if !pdf.is_file() {
        return Err(format!("not on disk: {}", pdf.display()));
    }
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
