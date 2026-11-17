//! The folder names themselves: the only place they are spelled.

use super::{OwnKind, SyncedFile, SyncedKind};

/// One library layout. Every function in this module reads `CURRENT`; a test
/// holds the target layout beside it, so the same rules are checked on both.
pub(super) struct Shape {
    /// The folder at a subject's root that holds everything synced, or `None`
    /// when the synced folders sit at the root themselves.
    pub synced_root: Option<&'static str>,
    /// Each synced kind's folder, under `synced_root`.
    pub synced_dirs: &'static [(SyncedKind, &'static str)],
    /// Synced single files, under `synced_root`.
    pub synced_files: &'static [(SyncedFile, &'static str)],
    /// Where each own kind is created, under the subject root (`""` is the
    /// root itself).
    pub own_dirs: &'static [(OwnKind, &'static str)],
    /// Whether an own file's kind is its folder's (`own_dirs`, and the own
    /// zone is just those folders) or its extension's (anywhere outside the
    /// synced and hidden folders).
    pub own_kind_by_folder: bool,
    /// Folders at a subject's root that hold app state, not library files.
    pub hidden: &'static [&'static str],
    /// The subject's agent workspace, under its root.
    pub agents: &'static str,
    /// The folder beside a note that holds its pictures, and the prefix of
    /// the `![](…)` links the note carries.
    pub note_assets: &'static str,
    /// The data-dir folder holding one folder per lecture recording, or
    /// `None` when recordings sit inside their subject.
    pub recordings_root: Option<&'static str>,
}

pub(super) const CURRENT: &Shape = &TODAY;

pub(super) const TODAY: Shape = Shape {
    synced_root: None,
    synced_dirs: &[
        (SyncedKind::Files, "files"),
        (SyncedKind::Media, "files"),
        (SyncedKind::Lessons, "lessons"),
        (SyncedKind::Discussion, "ed"),
        (SyncedKind::Pages, "pages"),
        (SyncedKind::Assignments, "assignments"),
        (SyncedKind::Quizzes, "quizzes"),
        (SyncedKind::Announcements, "announcements"),
        (SyncedKind::Modules, "modules"),
        (SyncedKind::Images, "images"),
    ],
    synced_files: &[
        (SyncedFile::Home, "home.md"),
        (SyncedFile::Syllabus, "syllabus.md"),
    ],
    own_dirs: &[
        (OwnKind::Upload, "uploads"),
        (OwnKind::Document, "documents"),
        (OwnKind::Canvas, "canvases"),
        (OwnKind::Notebook, "notebooks"),
    ],
    own_kind_by_folder: true,
    hidden: &["agents"],
    agents: "agents",
    note_assets: "assets",
    recordings_root: Some("lectures"),
};

impl Shape {
    pub fn synced_dir(&self, kind: SyncedKind) -> &'static str {
        self.synced_dirs
            .iter()
            .find(|(k, _)| *k == kind)
            .map(|(_, dir)| *dir)
            .expect("every synced kind has a folder")
    }

    pub fn synced_file(&self, file: SyncedFile) -> &'static str {
        self.synced_files
            .iter()
            .find(|(f, _)| *f == file)
            .map(|(_, name)| *name)
            .expect("every synced file has a name")
    }

    pub fn own_dir(&self, kind: OwnKind) -> &'static str {
        self.own_dirs
            .iter()
            .find(|(k, _)| *k == kind)
            .map(|(_, dir)| *dir)
            .expect("every own kind has a folder")
    }

    /// `parts` joined under `synced_root`, as a subject-relative path.
    pub fn under_synced(&self, parts: &[&str]) -> String {
        self.synced_root
            .into_iter()
            .chain(parts.iter().copied())
            .filter(|p| !p.is_empty())
            .collect::<Vec<_>>()
            .join("/")
    }
}
