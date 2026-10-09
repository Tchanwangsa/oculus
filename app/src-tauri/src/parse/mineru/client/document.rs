//! One document in flight: what a batch worker fills in and the caller parked in `wait` reads out.

use super::archive::page_count;
use super::{BACKEND, SKIP_CHECK};
use crate::parse::{ParseError, ParsePage, Phase, Progress, Skips};
use crate::providers::ratelimit::hold;
use serde_json::Value;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering as AtomicOrdering;
use std::sync::{Arc, Condvar, Mutex};

/// What a batch worker fills in, and what the caller parked in `wait` reads
/// out. The progress callback is not `Send`, so the worker records numbers and
/// the caller's thread calls it.
pub struct CloudDocument {
    pdf: PathBuf,
    pub(super) images_dir: PathBuf,
    pub(super) images_rel: String,
    /// The PDF's size when queued: batching and upload order go by it.
    bytes: u64,
    /// Set when the caller stopped waiting on a skip, so the batch drops this
    /// document even after the mark is cleared for a fresh parse.
    abandoned: AtomicBool,
    state: Mutex<DocumentState>,
    changed: Condvar,
}

#[derive(Default)]
pub(super) struct DocumentState {
    /// `None` until its batch is submitted; nothing is reported before then.
    phase: Option<Phase>,
    /// Across all of this document's PUTs in the current submit.
    bytes_done: u64,
    bytes_total: u64,
    total_pages: u32,
    /// The PDF's page count once read, kept apart from `total_pages` so
    /// counting early reports no progress before the batch runs.
    counted_pages: Option<u32>,
    /// data_id → pages MinerU says it has extracted for that task.
    task_pages: HashMap<String, u32>,
    /// The content-list items of every finished task, rebased to absolute
    /// page indices.
    content: Vec<Value>,
    source_images: PathBuf,
    outcome: Option<Result<DocumentOutput, ParseError>>,
}

/// What one document's parse produced, before the seam turns it into a record.
#[derive(Debug)]
pub struct DocumentOutput {
    pub pages: Vec<ParsePage>,
    pub image_count: u32,
    pub total_pages: u32,
}

impl CloudDocument {
    pub fn new(pdf: &Path, images_dir: &Path, images_rel: &str) -> Arc<Self> {
        let bytes = fs::metadata(pdf).map(|m| m.len()).unwrap_or(0);
        Self::sized(pdf, images_dir, images_rel, bytes)
    }

    /// `new` with the size given rather than read, for the batching tests.
    pub(in crate::parse::mineru) fn sized(
        pdf: &Path,
        images_dir: &Path,
        images_rel: &str,
        bytes: u64,
    ) -> Arc<Self> {
        Arc::new(Self {
            pdf: pdf.to_path_buf(),
            images_dir: images_dir.to_path_buf(),
            images_rel: images_rel.to_string(),
            bytes,
            abandoned: AtomicBool::new(false),
            state: Mutex::new(DocumentState::default()),
            changed: Condvar::new(),
        })
    }

    pub fn path(&self) -> &Path {
        &self.pdf
    }

    pub fn bytes(&self) -> u64 {
        self.bytes
    }

    /// The user skipped this PDF (`parse::Skips`), or its caller has left.
    pub fn cancelled(&self) -> bool {
        self.abandoned.load(AtomicOrdering::SeqCst) || Skips::shared().is_marked(&self.pdf)
    }

    /// Park until the batch this document travelled in has an answer for it,
    /// delivering progress on this thread as it changes. A skip ends the wait
    /// within `SKIP_CHECK`; the batch drops the document on its own.
    pub fn wait(&self, on_progress: &dyn Fn(Progress)) -> Result<DocumentOutput, ParseError> {
        let mut state = hold(&self.state);
        let mut last: Option<Progress> = None;
        loop {
            if let Some(outcome) = state.outcome.take() {
                return outcome;
            }
            if self.cancelled() {
                self.abandoned.store(true, AtomicOrdering::SeqCst);
                return Err(ParseError::Cancelled);
            }
            if let Some(now) = state.progress() {
                if last.map_or(true, |seen| !same_progress(&seen, &now)) {
                    // Events coalesce here, so an upload this thread last saw
                    // part-way still ends at 100% before processing.
                    let unfinished = last.map_or(true, |seen| match seen.phase {
                        Phase::UploadWait => true,
                        Phase::Uploading => seen.bytes_done < seen.bytes_total,
                        Phase::Processing => false,
                    });
                    last = Some(now);
                    drop(state);
                    if unfinished && now.phase == Phase::Processing && now.bytes_total > 0 {
                        on_progress(Progress {
                            phase: Phase::Uploading,
                            bytes_done: now.bytes_total,
                            ..now
                        });
                    }
                    on_progress(now);
                    state = hold(&self.state);
                    continue;
                }
            }
            state = self
                .changed
                .wait_timeout(state, SKIP_CHECK)
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .0;
        }
    }

    /// Hand this document its answer. First writer wins: a worker that fails
    /// one document and then panics must not overwrite the real reason.
    pub fn finish(&self, outcome: Result<DocumentOutput, ParseError>) {
        let mut state = hold(&self.state);
        if state.outcome.is_none() {
            state.outcome = Some(outcome);
        }
        drop(state);
        self.changed.notify_all();
    }

    /// Read the PDF's page count once. The caller's thread does it before
    /// submitting, so a PDF that crashes `hayro-syntax` fails alone, not its
    /// batch.
    pub(super) fn count_pages(&self) -> Result<u32, ParseError> {
        if let Some(pages) = hold(&self.state).counted_pages {
            return Ok(pages);
        }
        let pages = page_count(&self.pdf)?;
        hold(&self.state).counted_pages = Some(pages);
        Ok(pages)
    }

    pub(super) fn set_total_pages(&self, pages: u32) {
        hold(&self.state).total_pages = pages;
        self.changed.notify_all();
    }

    /// Submitted: this many bytes wait for their turn to upload.
    pub(super) fn await_upload(&self, bytes_total: u64) {
        let mut state = hold(&self.state);
        state.phase = Some(Phase::UploadWait);
        state.bytes_done = 0;
        state.bytes_total = bytes_total;
        drop(state);
        self.changed.notify_all();
    }

    pub(super) fn set_phase(&self, phase: Phase) {
        hold(&self.state).phase = Some(phase);
        self.changed.notify_all();
    }

    pub(super) fn bytes_done(&self) -> u64 {
        hold(&self.state).bytes_done
    }

    pub(super) fn set_bytes_done(&self, bytes: u64) {
        let mut state = hold(&self.state);
        state.bytes_done = bytes.min(state.bytes_total);
        drop(state);
        self.changed.notify_all();
    }

    pub(super) fn total_pages(&self) -> u32 {
        hold(&self.state).total_pages
    }

    pub(super) fn set_source_images(&self, dir: PathBuf) {
        hold(&self.state).source_images = dir;
    }

    pub(super) fn source_images(&self) -> PathBuf {
        hold(&self.state).source_images.clone()
    }

    /// Monotonic per task, clamped to its length: MinerU's `extracted_pages`
    /// can go backwards between polls.
    pub(super) fn report(&self, data_id: &str, done: u32, page_count: u32) {
        let mut state = hold(&self.state);
        let slot = state.task_pages.entry(data_id.to_string()).or_default();
        *slot = (*slot).max(done.min(page_count));
        drop(state);
        self.changed.notify_all();
    }

    pub(super) fn push_content(&self, items: Vec<Value>) {
        hold(&self.state).content.extend(items);
    }

    pub(super) fn take_content(&self) -> Vec<Value> {
        std::mem::take(&mut hold(&self.state).content)
    }
}

impl DocumentState {
    /// The sum over this document's tasks, never more than its length.
    fn pages_done(&self) -> u32 {
        self.task_pages.values().sum::<u32>().min(self.total_pages)
    }

    fn progress(&self) -> Option<Progress> {
        Some(Progress {
            pages_done: self.pages_done(),
            total_pages: self.total_pages,
            backend: BACKEND,
            phase: self.phase?,
            bytes_done: self.bytes_done,
            bytes_total: self.bytes_total,
        })
    }
}

pub(super) fn same_progress(a: &Progress, b: &Progress) -> bool {
    (
        a.phase,
        a.pages_done,
        a.total_pages,
        a.bytes_done,
        a.bytes_total,
    ) == (
        b.phase,
        b.pages_done,
        b.total_pages,
        b.bytes_done,
        b.bytes_total,
    )
}
