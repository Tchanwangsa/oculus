//! The app's Studio libraries, under `<data dir>/studio/`: the global
//! library at `library/` and each subject's at `subjects/<CODE>/`, beside
//! the videos (`super::videos`). The starter set is seeded into the global
//! library on every load, missing files only, so an edited file is never
//! replaced. `--reset` (the CLI's `components seed --reset`) writes the
//! starter text over the edits, with each old text kept in `.history`
//! first. `promote` copies a video's own component or theme into a library
//! through the same snapshotting write.

mod promote;
#[cfg(test)]
mod tests;

use crate::library::paths::{data_dir, studio_dir};
use std::path::{Path, PathBuf};
use studio_core::{seed_library, Library, Project, StudioError};

pub use promote::{promote_component, promote_theme, Promoted, Target};

/// `<data dir>/studio/library`.
pub fn global_dir() -> PathBuf {
    studio_dir(&data_dir()).join("library")
}

/// `<data dir>/studio/subjects`, whose `<CODE>/` folders are the subjects'
/// libraries.
pub fn subjects_dir() -> PathBuf {
    studio_dir(&data_dir()).join("subjects")
}

/// The libraries at `global` and `subjects`, with the starter set seeded
/// into `global` first (see the module docs for `reset`).
pub fn libraries_at(global: &Path, subjects: &Path, reset: bool) -> Result<Library, StudioError> {
    seed_library(global, reset)?;
    Ok(Library {
        global: Some(global.to_owned()),
        subjects: Some(subjects.to_owned()),
        subject: None,
        theme: None,
    })
}

/// [`libraries_at`] for the app's data directory.
pub fn libraries(reset: bool) -> Result<Library, StudioError> {
    libraries_at(&global_dir(), &subjects_dir(), reset)
}

/// The project at `dir`, loaded over the app's libraries. `subject` and
/// `theme`, when given, replace video.json's for this load.
pub fn open(
    dir: &Path,
    subject: Option<String>,
    theme: Option<String>,
) -> Result<Project, StudioError> {
    Project::load_with(dir, &with(subject, theme)?)
}

/// [`open`] without reading the narration manifest, for what never needs
/// it (`gallery`): a missing or unreadable manifest does not stop it.
pub fn open_unnarrated(
    dir: &Path,
    subject: Option<String>,
    theme: Option<String>,
) -> Result<Project, StudioError> {
    Project::load_unnarrated_with(dir, &with(subject, theme)?)
}

/// The app's libraries with `subject` and `theme` replacing video.json's.
fn with(subject: Option<String>, theme: Option<String>) -> Result<Library, StudioError> {
    let mut library = libraries(false)?;
    library.subject = subject;
    library.theme = theme;
    Ok(library)
}
