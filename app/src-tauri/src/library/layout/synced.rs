//! What the sync writes inside a subject: one folder per kind, plus the
//! course's home page and syllabus.

use std::path::{Path, PathBuf};

use super::roots::course_root;
use super::shape::CURRENT;

/// A folder the sync fills. `Media` is a module's videos and Echo360 page
/// embeds; `Discussion` is Ed threads.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SyncedKind {
    Files,
    Media,
    Lessons,
    Discussion,
    Pages,
    Assignments,
    Quizzes,
    Announcements,
    Modules,
    Images,
}

impl SyncedKind {
    /// The `files.category` its rows carry.
    pub fn category(self) -> &'static str {
        match self {
            Self::Files | Self::Media => "file",
            Self::Lessons => "lessons",
            Self::Discussion => "ed",
            Self::Pages => "page",
            Self::Assignments => "assignment",
            Self::Quizzes => "quiz",
            Self::Announcements => "announcement",
            Self::Modules => "module",
            Self::Images => "image",
        }
    }
}

/// A single file the sync writes at the top of the synced folders.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SyncedFile {
    Home,
    Syllabus,
}

impl SyncedFile {
    pub fn category(self) -> &'static str {
        match self {
            Self::Home => "home",
            Self::Syllabus => "syllabus",
        }
    }
}

/// `<kind's folder>/<name>`, subject-relative: what the sync's writers take.
/// `name` may itself hold folders (`1-intro/03-q1a`).
pub fn synced(kind: SyncedKind, name: &str) -> String {
    CURRENT.under_synced(&[CURRENT.synced_dir(kind), name])
}

/// A kind's folder, subject-relative.
pub fn synced_dir(kind: SyncedKind) -> String {
    CURRENT.under_synced(&[CURRENT.synced_dir(kind)])
}

/// `home.md` or `syllabus.md`, subject-relative.
pub fn synced_file(file: SyncedFile) -> String {
    CURRENT.under_synced(&[CURRENT.synced_file(file)])
}

/// A kind's folder for one subject, on disk.
pub fn synced_root(data_dir: &Path, code: &str, kind: SyncedKind) -> PathBuf {
    course_root(data_dir, code).join(synced_dir(kind))
}
