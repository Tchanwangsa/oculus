//! What the one-shot move from `LEGACY` into this layout asks of it
//! (`library::migrate`): where each old folder of a subject goes, and the
//! names the move gives what has no place of its own.

use super::shape::{Shape, CURRENT, LEGACY};
use super::{OwnKind, SyncedKind};

/// The folder at a subject's root that takes what agents wrote into the old
/// agent folder: the student's to keep, so not hidden.
pub const AGENT_NOTES: &str = "Agent notes";

/// How the move treats one old folder of a subject.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FolderMoveKind {
    /// Renamed whole into `Course/`.
    Synced,
    /// One synced file (`home.md`, `syllabus.md`), renamed into `Course/`.
    SyncedFile,
    /// An own kind's folder: its entries go to the new folder (the subject's
    /// root) one by one, since others may already be there.
    Own,
    /// A notes folder's pictures folder, renamed to the hidden one.
    NoteAssets,
    /// The subject's agent folder, sorted by what each entry is.
    Agents,
}

/// One old folder (or synced file) of a subject and where it goes, both
/// subject-relative; `""` is the subject's root.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FolderMove {
    pub old: String,
    pub new: String,
    pub kind: FolderMoveKind,
}

/// Every old folder of a subject with its new place. `Media` shared `Files`'s
/// folder before, so it has no rule of its own: the move picks videos out of
/// `Files` one by one.
pub fn legacy_folder_moves() -> Vec<FolderMove> {
    let mut moves: Vec<FolderMove> = Vec::new();
    for (kind, old) in LEGACY.synced_dirs {
        if *kind == SyncedKind::Media || moves.iter().any(|m| m.old == *old) {
            continue;
        }
        moves.push(FolderMove {
            old: LEGACY.under_synced(&[*old]),
            new: CURRENT.under_synced(&[CURRENT.synced_dir(*kind)]),
            kind: FolderMoveKind::Synced,
        });
    }
    for (file, old) in LEGACY.synced_files {
        moves.push(FolderMove {
            old: LEGACY.under_synced(&[*old]),
            new: CURRENT.under_synced(&[CURRENT.synced_file(*file)]),
            kind: FolderMoveKind::SyncedFile,
        });
    }
    for (kind, old) in LEGACY.own_dirs {
        moves.push(FolderMove {
            old: old.to_string(),
            new: CURRENT.own_dir(*kind).to_string(),
            kind: FolderMoveKind::Own,
        });
    }
    moves.push(FolderMove {
        old: note_assets_of(&LEGACY),
        new: note_assets_of(CURRENT),
        kind: FolderMoveKind::NoteAssets,
    });
    moves.push(FolderMove {
        old: LEGACY.agents.to_string(),
        new: CURRENT.agents.to_string(),
        kind: FolderMoveKind::Agents,
    });
    moves
}

/// The pictures folder of the notes folder, subject-relative.
fn note_assets_of(shape: &Shape) -> String {
    [shape.own_dir(OwnKind::Document), shape.note_assets]
        .into_iter()
        .filter(|p| !p.is_empty())
        .collect::<Vec<_>>()
        .join("/")
}

/// A note's pictures folder by its old name and its new one: a notes folder
/// below the top keeps its own, renamed in place.
pub fn note_assets_rename() -> (&'static str, &'static str) {
    (LEGACY.note_assets, CURRENT.note_assets)
}

/// The data-dir folder that held one folder per lecture recording, named by
/// its Echo360 media id.
pub fn legacy_recordings_root() -> &'static str {
    LEGACY
        .recordings_root()
        .expect("the old layout kept recordings in the data dir")
}

/// The old `Files` folder, subject-relative: where module and page-embed
/// videos sat beside the documents.
pub fn legacy_files_dir() -> String {
    LEGACY.under_synced(&[LEGACY.synced_dir(SyncedKind::Files)])
}

/// The name a student's folder takes when its own is one the layout keeps
/// (`reserved_name`): `Course` becomes `Course (mine)`.
pub fn reserved_rename(name: &str) -> String {
    format!("{name} (mine)")
}
