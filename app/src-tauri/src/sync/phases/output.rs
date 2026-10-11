//! Writing artifacts to the library and announcing them.

use crate::library::paths;
use crate::parse;
use crate::sync::parse::parse_pdf;
use crate::sync::{Engine, FileEvent, Subject};

impl Engine {
    /// Write one artifact and announce it. Returns `courses/CODE/...`.
    pub(in crate::sync) fn write(
        &self,
        c: &Subject,
        rel_path: &str,
        data: &[u8],
        canvas_id: Option<i64>,
    ) -> Result<String, String> {
        self.write_from(c, rel_path, data, canvas_id, None)
    }

    pub(in crate::sync) fn write_from(
        &self,
        c: &Subject,
        rel_path: &str,
        data: &[u8],
        canvas_id: Option<i64>,
        source_url: Option<String>,
    ) -> Result<String, String> {
        self.store(c, rel_path, data, canvas_id, source_url)
            .map(|(rel, _)| rel)
    }

    /// `write_from`, also saying what the write did to the file on disk.
    pub(in crate::sync) fn store(
        &self,
        c: &Subject,
        rel_path: &str,
        data: &[u8],
        canvas_id: Option<i64>,
        source_url: Option<String>,
    ) -> Result<(String, paths::WriteAction), String> {
        let (rel, size, action) =
            paths::write_course_bytes(&self.data_dir, &c.code, rel_path, data)?;
        // Purge the stale parse before the trigger below, or its skip check
        // would keep serving the old markdown and vectors.
        if action == paths::WriteAction::Updated {
            paths::purge_parse_artifacts(&self.data_dir, &rel);
        }
        self.reporter.file(&FileEvent {
            subject_id: c.id,
            code: c.code.clone(),
            relative_path: rel.clone(),
            size_bytes: size,
            category: paths::category_from_path(rel_path).to_string(),
            canvas_id,
            source_url,
            action: action.as_str(),
        });

        if self.parse_pdfs && paths::is_pdf(rel_path) {
            self.trigger_parse(&rel, c.id);
        }
        Ok((rel, action))
    }

    /// A spreadsheet's text, at once: it takes moments and starts no parse.
    /// A failure is the row's parse error; the bytes stay in the manifest,
    /// since a re-download cannot change them.
    pub(in crate::sync) fn convert_sheet(&self, code: &str, rel: &str, subject_id: i64) {
        if let Err(e) = crate::pages::sheets::index(&self.data_dir, rel, subject_id) {
            let name = rel.rsplit('/').next().unwrap_or(rel);
            self.reporter
                .log("warning", code, &format!("{name}: no text — {e}"));
        }
    }

    /// An Office original whose conversion failed has no PDF to parse. A
    /// derived PDF from older bytes is deleted with its artifacts, or it would
    /// be parsed as current; the failure is the row's terminal parse status.
    /// Identical bytes keep a sibling that is still theirs.
    pub(in crate::sync) fn conversion_failed(
        &self,
        rel: &str,
        action: paths::WriteAction,
        subject_id: i64,
    ) {
        let Some(pdf_rel) = paths::doc_pdf_rel(rel) else {
            return;
        };
        let derived = self.data_dir.join(&pdf_rel);
        if action == paths::WriteAction::Unchanged && derived.is_file() {
            return;
        }
        paths::purge_parse_artifacts(&self.data_dir, rel);
        let _ = std::fs::remove_file(&derived);
        if self.parse_pdfs {
            let error = parse::ParseError::Document {
                code: parse::CONVERSION_FAILED.into(),
            };
            parse::events::failed(rel, subject_id, &error);
        }
    }

    /// Start the parse without waiting for it. One detached thread per PDF,
    /// deliberately unpooled: concurrency belongs to the batcher
    /// (`parse/mineru/batch.rs`), and a gate here would split its batches.
    pub(in crate::sync) fn trigger_parse(&self, rel: &str, subject_id: i64) {
        let data_dir = self.data_dir.clone();
        let rel = rel.to_string();
        // Not worth failing a scrape over — `oculus index` re-runs the parse.
        let _ = std::thread::Builder::new()
            .name("oculus-parse".into())
            .spawn(move || match parse_pdf(&data_dir, &rel, subject_id) {
                Ok(summary) => eprintln!("[oculus] parse-pdf {rel}: {summary}"),
                Err(e) => eprintln!("[oculus] parse-pdf {rel}: {e}"),
            });
    }
}
