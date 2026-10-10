//! Skipping a file's parse.

use super::ParseError;
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

/// PDFs the user skipped this session, keyed like `InFlight`. Memory only:
/// across restarts the frontend's `files.parse_status = 'skipped'` holds it.
/// Engines poll `is_marked` and end the parse as `ParseError::Cancelled`.
pub struct Skips {
    marked: Mutex<BTreeSet<PathBuf>>,
}

impl Skips {
    pub const fn new() -> Self {
        Self {
            marked: Mutex::new(BTreeSet::new()),
        }
    }

    pub fn shared() -> &'static Skips {
        static SHARED: Skips = Skips::new();
        &SHARED
    }

    pub fn mark(&self, pdf: &Path) {
        self.marked
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .insert(pdf.to_path_buf());
    }

    pub fn clear(&self, pdf: &Path) {
        self.marked
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .remove(pdf);
    }

    pub fn is_marked(&self, pdf: &Path) -> bool {
        self.marked
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .contains(pdf)
    }
}

/// `Err(Cancelled)` when the user skipped this PDF.
pub fn check_skipped(pdf: &Path) -> Result<(), ParseError> {
    if Skips::shared().is_marked(pdf) {
        return Err(ParseError::Cancelled);
    }
    Ok(())
}
