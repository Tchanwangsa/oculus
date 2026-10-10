//! Image staging: extracted images are swapped into place only on success.

use super::{images_dir_for, ParseError};
use std::fs;
use std::path::{Path, PathBuf};

/// A scratch directory for extracted images, swapped into place only when the
/// whole parse succeeded. Dropped without committing, it deletes itself and the
/// previous parse's artifacts are untouched: a failure never leaves a file
/// less parsed than it was.
pub struct ImageStaging {
    staged: PathBuf,
    destination: PathBuf,
    rel: String,
}

impl ImageStaging {
    /// Stage beside the PDF, never in the system temp dir: the swap is a
    /// rename, which fails across filesystems. The dot prefix keeps it out of
    /// file listings.
    pub fn begin(pdf: &Path) -> Result<Self, ParseError> {
        let destination = images_dir_for(pdf);
        let name = destination
            .file_name()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_default();
        let stamp = crate::runtime::clock::now_nanos();
        let scratch = pdf.with_file_name(format!(".{name}-staging-{}-{stamp}", std::process::id()));
        fs::create_dir_all(scratch.join(&name))
            .map_err(|e| ParseError::Io(format!("stage {}: {e}", scratch.display())))?;
        Ok(Self {
            staged: scratch.join(&name),
            destination,
            rel: name,
        })
    }

    /// Where the backend writes extracted images.
    pub fn dir(&self) -> &Path {
        &self.staged
    }

    /// The link prefix for the markdown: the *final* directory's name, not
    /// the scratch one.
    pub fn rel(&self) -> &str {
        &self.rel
    }

    /// Swap the staged images into place; `Drop` then clears the wrapper.
    /// Replaced, not merged: a re-parse renumbers crops.
    pub(super) fn commit(self) -> Result<(), ParseError> {
        fs::remove_dir_all(&self.destination).ok();
        fs::rename(&self.staged, &self.destination)
            .map_err(|e| ParseError::Io(format!("swap in {}: {e}", self.destination.display())))
    }
}

impl Drop for ImageStaging {
    fn drop(&mut self) {
        if let Some(root) = self.staged.parent() {
            fs::remove_dir_all(root).ok();
        }
    }
}
