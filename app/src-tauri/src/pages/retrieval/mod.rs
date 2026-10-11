//! Semantic page retrieval over page *images* (see `docs/retrieval.md`).
//!
//! `embed::backend` embeds each page -> blobs land in `pages` beside the
//! page's markdown -> a query is embedded by the same backend and ranked by
//! dot product in a brute-force scan -> hits carry markdown and (file, page).
//!
//! **One space, or the ranking is noise.** Every scan filters on
//! `pages.embed_model` and `pages.embed_dim`: a dot product across two models
//! still sorts, confidently and meaninglessly. Vectors from a retired model
//! stay in the table but are never scanned, and `IndexStats` counts them apart.

pub(crate) mod commands;
#[cfg(test)]
mod fts_tests;
mod ingest;
mod plumbing;
mod search;
mod stats;
#[cfg(test)]
mod tests;

pub use ingest::{ingest, ingest_reporting};
pub use search::{search, search_in};
pub use stats::stats;

use std::sync::Arc;

use serde::Serialize;

use crate::embed;

/// The lexical half of search: an FTS5 index over `pages.markdown`, for exact
/// words. Queried from the frontend (`searchPageText` in `app/src/lib/db/search.ts`).
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

/// Upserts name `markdown` even when its value is unchanged. Keep those
/// writes from deleting and reinserting the same FTS terms (migration 38).
pub const PAGES_FTS_CHANGED_SQL: &str = r#"
DROP TRIGGER pages_fts_au;
CREATE TRIGGER pages_fts_au AFTER UPDATE OF markdown ON pages
WHEN old.markdown IS NOT new.markdown BEGIN
    INSERT INTO pages_fts(pages_fts, rowid, markdown)
    VALUES ('delete', old.id, old.markdown);
    INSERT INTO pages_fts(rowid, markdown) VALUES (new.id, new.markdown);
END;
"#;

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
        Self {
            message,
            kind: None,
            retryable: None,
            latching: None,
        }
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
