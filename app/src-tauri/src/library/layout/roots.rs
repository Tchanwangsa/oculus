//! The data-dir folders the library occupies, and a subject's root.

use std::path::{Path, PathBuf};

use super::shape::CURRENT;
use crate::library::paths::safe_dir;

/// The data-dir folder holding one folder per subject.
pub const COURSES_DIR: &str = "courses";

/// The data-dir folders the library occupies: what an agent may read, the
/// media server may serve, and a picture may be imported from.
pub fn library_roots() -> Vec<&'static str> {
    std::iter::once(COURSES_DIR)
        .chain(CURRENT.recordings_root)
        .collect()
}

/// `<data dir>/courses`.
pub fn courses_dir(data_dir: &Path) -> PathBuf {
    data_dir.join(COURSES_DIR)
}

/// `courses/<code>`, data-dir-relative. The code is sanitised as the folder
/// was named (`safe_dir`).
pub fn course_rel(code: &str) -> String {
    format!("{COURSES_DIR}/{}", safe_dir(code))
}

/// A subject's root folder.
pub fn course_root(data_dir: &Path, code: &str) -> PathBuf {
    courses_dir(data_dir).join(safe_dir(code))
}

/// `courses/<code>/<inner>`, data-dir-relative. `inner` is subject-relative
/// and taken as it is: sanitising is the caller's (`course_rel_path`).
pub fn in_course(code: &str, inner: &str) -> String {
    format!("{}/{inner}", course_rel(code))
}

/// A data-dir-relative path split into its subject folder and the path
/// inside it: `courses/X/files/a.pdf` → `("X", "files/a.pdf")`. `None`
/// outside a subject, or for the subject folder itself.
pub fn split_course(rel: &str) -> Option<(&str, &str)> {
    let rest = rel.strip_prefix(COURSES_DIR)?.strip_prefix('/')?;
    let (code, inner) = rest.split_once('/')?;
    (!code.is_empty() && !inner.is_empty()).then_some((code, inner))
}

/// The path inside its subject, or the path itself outside one — for
/// display and for module TOC links.
pub fn within_course(rel: &str) -> &str {
    split_course(rel).map_or(rel, |(_, inner)| inner)
}
