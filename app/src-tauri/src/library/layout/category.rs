//! A row's `files.category`, from its path inside the subject.

use super::shape::{Shape, CURRENT};
use super::{OwnKind, SyncedKind};

/// Every category `category_of` can return, in reading order. The
/// `--category` flags validate against this one list; a test fails if it
/// drifts from the rules. Mirrored for the frontend by
/// `app/src/lib/files/libraryLayout.ts`, checked against the same fixture.
pub const CATEGORIES: &[&str] = &[
    "home",
    "syllabus",
    "upload",
    "document",
    "canvas",
    "notebook",
    "page",
    "assignment",
    "quiz",
    "announcement",
    "ed",
    "lessons",
    "file",
    "module",
    "image",
    "other",
];

/// The category of a subject-relative path (`files/a.pdf`, `home.md`).
/// Synced files go by their folder; own files by their folder or their
/// extension, as the layout says; anything else is `other`.
pub fn category_of(inner: &str) -> &'static str {
    CURRENT.category_of(inner)
}

impl Shape {
    pub(super) fn category_of(&self, inner: &str) -> &'static str {
        let synced = match self.synced_root {
            None => Some(inner),
            Some(root) => inner
                .strip_prefix(root)
                .and_then(|rest| rest.strip_prefix('/')),
        };
        if let Some(rest) = synced {
            if let Some((file, _)) = self.synced_files.iter().find(|(_, name)| rest == *name) {
                return file.category();
            }
            if let Some(kind) = self.synced_kind_of(rest) {
                return kind.category();
            }
        }
        if self.own_kind_by_folder {
            let Some((first, _)) = inner.split_once('/') else {
                return "other";
            };
            return self
                .own_dirs
                .iter()
                .find(|(_, dir)| *dir == first)
                .map_or("other", |(kind, _)| kind.category());
        }
        let first = inner.split('/').next().unwrap_or_default();
        if synced.is_some() || self.hidden.contains(&first) {
            return "other";
        }
        OwnKind::by_extension(inner).category()
    }

    fn synced_kind_of(&self, rest: &str) -> Option<SyncedKind> {
        let (first, _) = rest.split_once('/')?;
        self.synced_dirs
            .iter()
            .find(|(_, dir)| *dir == first)
            .map(|(kind, _)| *kind)
    }
}
