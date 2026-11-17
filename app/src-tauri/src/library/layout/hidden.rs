//! App state inside a subject: its agent workspace.

use std::path::{Path, PathBuf};

use super::roots::{course_rel, course_root};
use super::shape::CURRENT;

/// A subject's agent workspace (its `INSTRUCTIONS.md`, and the link to its
/// memories), on disk.
pub fn agents_dir(data_dir: &Path, code: &str) -> PathBuf {
    agents_under(&course_root(data_dir, code))
}

/// The agent workspace of the subject folder at `course_root`, for walks
/// that already hold the folder.
pub fn agents_under(course_root: &Path) -> PathBuf {
    course_root.join(CURRENT.agents)
}

/// The same folder, data-dir-relative.
pub fn agents_rel(code: &str) -> String {
    format!("{}/{}", course_rel(code), CURRENT.agents)
}
