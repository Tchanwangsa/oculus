//! PDF parsing from a sync or the CLI, and folding the result into `pages`.

use std::path::{Path, PathBuf};

use crate::library::paths;
use crate::parse;

/// What a finished `parse_pdf` did.
#[derive(Debug, Clone)]
pub struct ParseSummary {
    /// The PDF already had a current `.pages.json`; nothing was sent.
    pub skipped: bool,
    pub pages: u32,
    pub images: u32,
    /// Page rows written to `pages`. Zero with `skipped` false means no
    /// database or no file row yet; the artifacts are still on disk.
    pub pages_recorded: usize,
}

impl std::fmt::Display for ParseSummary {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.skipped {
            write!(f, "already parsed ({} pages", self.pages)?;
            if self.pages_recorded > 0 {
                write!(f, ", {} records folded in", self.pages_recorded)?;
            }
            return write!(f, ")");
        }
        write!(
            f,
            "{} pages, {} images, {} recorded",
            self.pages, self.images, self.pages_recorded
        )
    }
}

/// Parse a PDF, blocking (for minutes) until the artifacts are on disk.
/// Deliberately no timeout here — the client's `POLL_DEADLINE` is the only
/// one (see `docs/parsing.md`). Idempotent.
///
/// `rel_path` is the library file; for Office documents the bytes parsed are
/// its derived sibling PDF.
pub fn parse_pdf(
    data_dir: &Path,
    rel_path: &str,
    subject_id: i64,
) -> Result<ParseSummary, parse::ParseError> {
    parse_pdf_reporting(data_dir, rel_path, subject_id, false, &|_| {})
}

/// `parse_pdf` plus a progress callback, for the CLI (the app reads the
/// `parse-status` events). `reparse_outdated` parses a file again when its
/// record predates `PARSER_VERSION` (`oculus index --reparse`).
///
/// One parse per PDF at a time (`parse::InFlight`): a second caller waits,
/// then takes the already-parsed path. A panic becomes this file's `error`,
/// or its row would wait on a status that never comes.
pub fn parse_pdf_reporting(
    data_dir: &Path,
    rel_path: &str,
    subject_id: i64,
    reparse_outdated: bool,
    on_progress: &dyn Fn(parse::Progress),
) -> Result<ParseSummary, parse::ParseError> {
    let key = parse_key(data_dir, rel_path);
    let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _claim = parse::InFlight::shared().claim(&key);
        run_parse(
            data_dir,
            rel_path,
            subject_id,
            reparse_outdated,
            on_progress,
        )
    }))
    .unwrap_or_else(|_| {
        Err(parse::ParseError::Io(
            "the parser crashed on this file".into(),
        ))
    });
    match outcome {
        Ok(summary) => Ok(summary),
        Err(error) => {
            parse::events::failed(rel_path, subject_id, &error);
            Err(error)
        }
    }
}

/// The PDF a library file parses as: the key of `parse::InFlight` and
/// `parse::Skips`, and the path every engine is handed.
pub fn parse_key(data_dir: &Path, rel_path: &str) -> PathBuf {
    data_dir.join(paths::doc_pdf_rel(rel_path).unwrap_or_else(|| rel_path.to_string()))
}

fn run_parse(
    data_dir: &Path,
    rel_path: &str,
    subject_id: i64,
    reparse_outdated: bool,
    on_progress: &dyn Fn(parse::Progress),
) -> Result<ParseSummary, parse::ParseError> {
    // Local failures: `Io` is retryable, not latching, so a missing file never
    // stops the rest of the library parsing.
    let pdf_rel = paths::doc_pdf_rel(rel_path).ok_or_else(|| {
        parse::ParseError::Io(format!("{rel_path} has no PDF representation to parse"))
    })?;
    let pdf = data_dir.join(&pdf_rel);
    if !pdf.is_file() {
        return Err(parse::ParseError::Io(format!(
            "not on disk: {}",
            pdf.display()
        )));
    }

    // Decided under the `InFlight` claim, so a parse that just landed counts.
    // Nothing is purged first: the old record stands until the new one lands,
    // and `.emb.json` (page images, same page count) stays current.
    let reparse = reparse_outdated && parse::is_outdated(&pdf);
    if !reparse && parse::parse_mode(&pdf).is_some() {
        // An artifact on disk is no promise its page rows exist; backfill.
        let record = parse::read_record(&pdf);
        // The `.md` is derived from the record, so a lost one is rebuilt here.
        if let Some(record) = &record {
            record.restore_markdown(&pdf)?;
        }
        let pages = record.as_ref().map(|r| r.page_count).unwrap_or(0);
        let pages_recorded = match record {
            Some(record) => {
                backfill_pages(data_dir, rel_path, subject_id, &record).unwrap_or_else(|e| {
                    eprintln!("[oculus] parse-pdf {rel_path}: page records not backfilled: {e}");
                    0
                })
            }
            None => 0,
        };
        // A sweep that kicked this row is waiting for a terminal status.
        parse::events::parsed(rel_path, subject_id);
        return Ok(ParseSummary {
            skipped: true,
            pages,
            images: 0,
            pages_recorded,
        });
    }

    // Skipped by the user: nothing is sent. A parse already done stays done.
    parse::check_skipped(&pdf)?;
    parse::events::queued(rel_path, subject_id);

    let parser = parse::backend()?;
    parse::preflight(parser.as_ref())?;

    let staging = parse::ImageStaging::begin(&pdf)?;
    let output = parser.parse(&pdf, staging.dir(), staging.rel(), &|progress| {
        // `parse_skip` has already reported `skipped`; don't take it back.
        if parse::Skips::shared().is_marked(&pdf) {
            return;
        }
        parse::events::running(rel_path, subject_id, progress);
        on_progress(progress);
    })?;
    // A result that lands after a skip is discarded, never written.
    parse::check_skipped(&pdf)?;
    // `.pages.json` is the only evidence a parse finished; written last, atomically.
    output.write(&pdf, staging)?;

    // Best-effort: a database problem must not fail a successful parse;
    // `oculus index` folds it in later.
    let pages_recorded = match record_pages(data_dir, rel_path, subject_id, &output) {
        Ok(n) => n,
        Err(e) => {
            eprintln!("[oculus] parse-pdf {rel_path}: page records not written: {e}");
            0
        }
    };

    parse::events::parsed(rel_path, subject_id);
    Ok(ParseSummary {
        skipped: false,
        pages: output.page_count,
        images: output.image_count,
        pages_recorded,
    })
}

/// Fold an already-parsed file's record in only if it has no rows yet —
/// unlike `record_pages`, which always writes fresh text.
fn backfill_pages(
    data_dir: &Path,
    rel_path: &str,
    subject_id: i64,
    record: &parse::ParseOutput,
) -> Result<usize, String> {
    tauri::async_runtime::block_on(async move {
        let pool = crate::db::store::open(data_dir).await?;
        let result = async {
            let Some(file_id) = crate::db::store::file_id(&pool, subject_id, rel_path).await?
            else {
                return Ok(0);
            };
            if crate::db::store::page_count(&pool, file_id).await? > 0 {
                return Ok(0);
            }
            crate::db::store::upsert_pages(&pool, file_id, &record.pages).await
        }
        .await;
        pool.close().await;
        result
    })
}

/// Fold a finished parse into the `pages` table.
fn record_pages(
    data_dir: &Path,
    rel_path: &str,
    subject_id: i64,
    output: &parse::ParseOutput,
) -> Result<usize, String> {
    tauri::async_runtime::block_on(async move {
        let pool = crate::db::store::open(data_dir).await?;
        let result = match crate::db::store::file_id(&pool, subject_id, rel_path).await? {
            Some(file_id) => crate::db::store::upsert_pages(&pool, file_id, &output.pages).await,
            // A parse can outrun the row: the frontend writes `files` from
            // scrape events, and a CLI run may have no database write at all.
            None => Ok(0),
        };
        pool.close().await;
        result
    })
}
