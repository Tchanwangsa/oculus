//! The folder names themselves: the only place they are spelled.

use super::{OwnKind, SyncedFile, SyncedKind};

/// One library layout. Every function in this module reads `CURRENT`;
/// `LEGACY` is the layout before the Drive tree, kept so the one-shot move
/// can recognise old paths, and checked by the same rules in the tests.
pub struct Shape {
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
    /// Where a PDF's parse record and page vectors go, under the subject
    /// root, mirroring the PDF's own folder; `None` keeps them beside it.
    pub derived: Option<&'static str>,
    /// The folder beside a note that holds its pictures, and the prefix of
    /// the `![](…)` links the note carries.
    pub note_assets: &'static str,
    /// Where lecture recordings live.
    pub recordings: Recordings,
}

/// Where a layout keeps one folder per lecture recording.
#[derive(Clone, Copy)]
pub enum Recordings {
    /// A data-dir folder, one folder per Echo360 media id inside it.
    DataDir(&'static str),
    /// A synced folder in the recording's subject, one folder per recording
    /// named by its date and title.
    Synced(&'static str),
}

pub(super) const CURRENT: &Shape = &TARGET;

/// The Drive tree: `Course/` holds everything synced, the student's own files
/// sit beside it at any depth, and `.oculus/` is app state.
pub(super) const TARGET: Shape = Shape {
    synced_root: Some("Course"),
    synced_dirs: &[
        (SyncedKind::Files, "Files"),
        (SyncedKind::Media, "Media"),
        (SyncedKind::Lessons, "Lessons"),
        (SyncedKind::Discussion, "Discussion"),
        (SyncedKind::Pages, "Pages"),
        (SyncedKind::Assignments, "Assignments"),
        (SyncedKind::Quizzes, "Quizzes"),
        (SyncedKind::Announcements, "Announcements"),
        (SyncedKind::Modules, "Modules"),
        (SyncedKind::Images, "Images"),
    ],
    synced_files: &[
        (SyncedFile::Home, "home.md"),
        (SyncedFile::Syllabus, "syllabus.md"),
    ],
    own_dirs: &[
        (OwnKind::Upload, ""),
        (OwnKind::Document, ""),
        (OwnKind::Canvas, ""),
        (OwnKind::Notebook, ""),
    ],
    own_kind_by_folder: false,
    hidden: &[".oculus"],
    agents: ".oculus/agents",
    derived: Some(".oculus/derived"),
    note_assets: ".assets",
    recordings: Recordings::Synced("Recordings"),
};

/// The layout before the Drive tree: synced kinds at the subject root, one
/// folder per own kind, recordings in the data dir's `lectures/`.
pub const LEGACY: Shape = Shape {
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
    derived: None,
    note_assets: "assets",
    recordings: Recordings::DataDir("lectures"),
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

    /// The data-dir folder recordings live in, when they live outside the
    /// subjects.
    pub fn recordings_root(&self) -> Option<&'static str> {
        match self.recordings {
            Recordings::DataDir(root) => Some(root),
            Recordings::Synced(_) => None,
        }
    }
}
