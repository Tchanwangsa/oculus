//! One extraction task (a page range of one document), and the scope of a failure.

use crate::parse::ParseError;
use serde_json::{json, Value};
use std::path::PathBuf;
use std::sync::atomic::AtomicU64;
use std::sync::atomic::Ordering as AtomicOrdering;

/// One extraction task: a page range of one document. A document over
/// `MAX_PAGES_PER_TASK` becomes several tasks that each upload the whole PDF
/// and let the server take the range; the API has no "upload once".
pub(super) struct Task {
    pub(super) document: usize,
    pub(super) data_id: String,
    pub(super) upload_name: String,
    pub(super) source: PathBuf,
    pub(super) page_offset: u32,
    pub(super) page_count: u32,
    pub(super) page_ranges: Option<String>,
}

impl Task {
    pub(super) fn api_entry(&self) -> Value {
        let mut entry = json!({
            "name": self.upload_name,
            "data_id": self.data_id,
            "is_ocr": false,
        });
        if let Some(ranges) = &self.page_ranges {
            entry["page_ranges"] = Value::String(ranges.clone());
        }
        entry
    }
}

/// Who a failure belongs to. Nothing falls back, so one bad file must never
/// fail the rest of its batch.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Scope {
    Document,
    Batch,
}

/// Credentials, quota and a version mismatch are true of every file in the
/// batch — `ParseError::latching` is exactly that question, so it decides.
pub(super) fn scope_of(error: &ParseError) -> Scope {
    if error.latching() {
        Scope::Batch
    } else {
        Scope::Document
    }
}

pub(super) fn basename(path: &str) -> String {
    path.rsplit(['/', '\\']).next().unwrap_or(path).to_string()
}

/// Unique within a batch; MinerU echoes it back as the join key.
pub(super) fn data_id() -> String {
    use std::hash::{BuildHasher, Hasher};
    static SEQUENCE: AtomicU64 = AtomicU64::new(0);
    let sequence = SEQUENCE.fetch_add(1, AtomicOrdering::Relaxed);
    let nanos = crate::runtime::clock::now_nanos() as u64;
    let mut hasher = std::collections::hash_map::RandomState::new().build_hasher();
    hasher.write_u64(nanos);
    hasher.write_u64(sequence);
    hasher.write_u32(std::process::id());
    format!(
        "oculus-{:016x}{:016x}",
        nanos ^ (sequence << 40),
        hasher.finish()
    )
}
