//! The Canvas scrape engine. Modules drive the walk, and pages and files are
//! fetched through them, so nothing is downloaded twice. Lives in Rust, not a
//! WebView — see `docs/architecture.md`.
//!
//! Progress leaves through [`Reporter`]: the app forwards it as Tauri events,
//! the CLI prints it.

mod engine;
mod office;
mod parse;
mod phases;
mod render;
pub(crate) mod scrape;
pub(crate) mod subjects;
pub mod terms;
#[cfg(test)]
mod tests;

pub use engine::Engine;
pub(crate) use office::{office_ext_of, office_to_pdf};
pub use parse::{parse_key, parse_pdf, parse_pdf_reporting, ParseSummary};
pub use render::slug;

use std::collections::{HashMap, HashSet};

/// Types stored as-is and parsed as PDFs.
const DOWNLOADABLE_TYPES: &[&str] = &["application/pdf"];

/// Office formats kept as-is plus a LibreOffice-converted sibling PDF, mapped
/// to the extension the converter needs on its input file.
const OFFICE_TYPES: &[(&str, &str)] = &[
    (
        "application/vnd.openxmlformats-officedocument.presentationml.presentation",
        "pptx",
    ),
    (
        "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
        "docx",
    ),
    ("application/vnd.ms-powerpoint", "ppt"),
    ("application/msword", "doc"),
];

/// Spreadsheets, kept as-is plus their text (`crate::pages::sheets`) when the name
/// is a spreadsheet's too (`is_sheet_type`).
const SHEET_TYPES: &[&str] = &[
    "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
    "application/vnd.ms-excel.sheet.macroEnabled.12",
    "application/vnd.ms-excel",
    "application/vnd.oasis.opendocument.spreadsheet",
];

/// A wedged soffice must not hang the whole sync run.
const OFFICE_CONVERT_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(180);

/// Larger files are skipped rather than filling the disk with recordings.
/// Videos are exempt: a sync never downloads them (see [`is_video`]).
const MAX_FILE_BYTES: u64 = 100 * 1024 * 1024;

/// Extensions that mark an untyped upload as a video.
const VIDEO_EXTS: &[&str] = &["mp4", "mov", "m4v", "webm"];

const IMAGE_EXT: &[(&str, &str)] = &[
    ("image/png", "png"),
    ("image/jpeg", "jpg"),
    ("image/jpg", "jpg"),
    ("image/gif", "gif"),
    ("image/webp", "webp"),
    ("image/svg+xml", "svg"),
    ("image/bmp", "bmp"),
];

#[derive(Debug, Clone, serde::Serialize)]
pub struct Progress {
    pub done: usize,
    pub total: usize,
    pub course: String,
    pub phase: String,
    pub label: String,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct FileEvent {
    pub subject_id: i64,
    pub code: String,
    pub relative_path: String,
    pub size_bytes: u64,
    pub category: String,
    pub canvas_id: Option<i64>,
    /// Set for pages: the URL slug survives renames while the filename tracks
    /// the title, so matching a body link to the local copy needs this.
    pub source_url: Option<String>,
    /// `"new"`, `"updated"`, or `"unchanged"` — feeds the per-run sync history.
    pub action: &'static str,
}

/// Announced before a download, under the same `relative_path` the eventual
/// [`FileEvent`] carries, so the UI can show "downloading" for a new file.
#[derive(Debug, Clone, serde::Serialize)]
pub struct FileStart {
    pub subject_id: i64,
    pub code: String,
    pub relative_path: String,
    pub filename: String,
    pub size_bytes: u64,
}

/// The terminal event for a [`FileStart`] whose download or save failed, so
/// the file is not left "downloading" until the run ends. `error` is a short
/// sentence for the UI — never a URL, which for a download is signed.
#[derive(Debug, Clone, serde::Serialize)]
pub struct FileFailed {
    pub subject_id: i64,
    pub relative_path: String,
    pub error: String,
}

/// Where a run's side effects go. Default methods are no-ops.
pub trait Reporter: Send + Sync {
    fn progress(&self, _p: &Progress) {}
    fn file_start(&self, _f: &FileStart) {}
    fn file(&self, _f: &FileEvent) {}
    fn file_failed(&self, _f: &FileFailed) {}
    fn log(&self, _level: &str, _course: &str, _message: &str) {}
    /// Checked between items; a run stops at the next boundary once true.
    fn cancelled(&self) -> bool {
        false
    }
    /// The run stopped because Canvas rejected the session and oculus-keyd
    /// could not sign in again; `message` says why.
    fn canvas_expired(&self, _message: &str) {}
}

/// Discards everything.
pub struct Silent;
impl Reporter for Silent {}

#[derive(Debug, Clone)]
pub struct Subject {
    pub id: i64,
    pub code: String,
}

/// Canvas id → course-relative path of each assignment/quiz document, keyed
/// the way module items refer to them (`content_id`).
#[derive(Debug, Default)]
pub struct TaskDocs {
    assignments: HashMap<i64, String>,
    quizzes: HashMap<i64, String>,
}

/// What `fetch_file` did with one Canvas file.
#[derive(Debug, Clone, PartialEq)]
enum Fetched {
    /// On disk at this `courses/CODE/…` path.
    Saved(String),
    /// A video, listed but not downloaded; `rel` is where
    /// [`Engine::download_video`] puts it.
    Video { rel: String, canvas_id: i64 },
    /// Locked, unsupported, oversized or not served.
    Skipped,
}

/// The per-course link crawl: every converted body queues the pages and files
/// it references, and `crawl_links` drains them depth-first after the content
/// phases, so anything reachable from any scraped body lands on disk.
#[derive(Debug, Default)]
struct LinkCrawl {
    seen_pages: HashSet<String>,
    seen_files: HashSet<String>,
    /// Pending page slugs / file ids, popped LIFO.
    pages: Vec<String>,
    files: Vec<String>,
}

impl LinkCrawl {
    fn absorb(&mut self, (pages, files): (Vec<String>, Vec<String>)) {
        self.pages.extend(pages);
        self.files.extend(files);
    }
}

/// A course as Canvas describes it, plus whether it belongs to the newest term.
#[derive(Debug, Clone, serde::Serialize)]
pub struct Course {
    pub id: i64,
    pub code: String,
    pub name: String,
    pub term: Option<String>,
    pub workflow_state: String,
    pub is_current: bool,
}

impl Course {
    /// The shape the frontend's `upsertSubjects` reads.
    pub fn to_canvas_json(&self) -> serde_json::Value {
        serde_json::json!({
            "id": self.id,
            "course_code": self.code,
            "name": self.name,
            "workflow_state": self.workflow_state,
            "term": self.term.as_ref().map(|t| serde_json::json!({ "name": t })),
            "_oculus_is_current": self.is_current,
        })
    }
}

/// Which content categories a sync fetches; all on by default (the CLI
/// always syncs everything).
#[derive(Debug, Clone, serde::Deserialize)]
#[serde(default)]
pub struct SyncOptions {
    pub announcements: bool,
    pub assignments: bool,
    pub modules: bool,
    pub ed: bool,
}

impl Default for SyncOptions {
    fn default() -> Self {
        SyncOptions {
            announcements: true,
            assignments: true,
            modules: true,
            ed: true,
        }
    }
}
