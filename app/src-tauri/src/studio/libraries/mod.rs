//! The app's Studio libraries, in its data directory: the global library at
//! `studio/library/` and each course's `studio/` folder under `courses/`.
//! The starter set is seeded into the global library on every load, missing
//! files only, so an edited file is never replaced. `--reset` (the CLI's
//! `components seed --reset`) writes the starter text over the edits, with
//! each old text kept in `.history` first.

use crate::library::paths::data_dir;
use std::path::{Path, PathBuf};
use studio_core::{seed_library, Library, Project, StudioError};

/// `<data dir>/studio/library`.
pub fn global_dir() -> PathBuf {
    data_dir().join("studio").join("library")
}

/// `<data dir>/courses`, whose `<code>/studio/` folders are the subjects'
/// libraries.
pub fn courses_dir() -> PathBuf {
    data_dir().join("courses")
}

/// The libraries at `global` and `courses`, with the starter set seeded into
/// `global` first (see the module docs for `reset`).
pub fn libraries_at(global: &Path, courses: &Path, reset: bool) -> Result<Library, StudioError> {
    seed_library(global, reset)?;
    Ok(Library {
        global: Some(global.to_owned()),
        courses: Some(courses.to_owned()),
        subject: None,
        theme: None,
    })
}

/// [`libraries_at`] for the app's data directory.
pub fn libraries(reset: bool) -> Result<Library, StudioError> {
    libraries_at(&global_dir(), &courses_dir(), reset)
}

/// The project at `dir`, loaded over the app's libraries. `subject` and
/// `theme`, when given, replace video.json's for this load.
pub fn open(
    dir: &Path,
    subject: Option<String>,
    theme: Option<String>,
) -> Result<Project, StudioError> {
    let mut library = libraries(false)?;
    library.subject = subject;
    library.theme = theme;
    Project::load_with(dir, &library)
}

#[cfg(test)]
mod tests;
