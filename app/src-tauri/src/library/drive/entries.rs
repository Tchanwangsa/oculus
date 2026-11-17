//! What `list_folder` returns, in the shape the frontend reads.

use serde::Serialize;

use crate::library::layout::Zone;

/// One folder's contents. `rel` is `""` for the Drive root, whose folders are
/// the subjects.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FolderListing {
    pub rel: String,
    /// `"synced"` or `"own"`; the Drive root and a subject's folder count as
    /// synced, since Oculus makes them.
    pub zone: &'static str,
    /// True when a file or folder made here would be the student's own: the
    /// New menu and drops are offered only then.
    pub writable: bool,
    /// Sorted by name, numbers in numeric order.
    pub folders: Vec<FolderEntry>,
    /// Sorted by name; for a recursive listing, every file below `rel`.
    pub files: Vec<FileEntry>,
    /// Lecture recordings whose folders sit here, by date; empty when
    /// recursive.
    pub recordings: Vec<RecordingEntry>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FolderEntry {
    pub relative_path: String,
    pub name: String,
    pub zone: &'static str,
    /// Files and folders may be made inside it.
    pub writable: bool,
    /// The folder itself may be renamed, moved or deleted.
    pub editable: bool,
    /// The entries a listing of it would show.
    pub child_count: u32,
    /// Unix milliseconds.
    pub modified: Option<u64>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FileEntry {
    pub relative_path: String,
    pub name: String,
    pub zone: &'static str,
    /// The file may be renamed, moved or deleted.
    pub editable: bool,
    pub size: u64,
    /// Unix milliseconds.
    pub modified: Option<u64>,
}

/// A lecture whose recording folder sits in the listed folder: opens the
/// lecture page.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RecordingEntry {
    pub lecture_id: String,
    pub subject_id: i64,
    /// The recording's folder, data-dir-relative.
    pub relative_path: String,
    pub title: String,
    pub date: String,
}

/// The zone as the frontend spells it. `Hidden` and `Outside` never reach a
/// listing; they map to `"synced"` only so the type stays total.
pub fn zone_name(zone: Zone) -> &'static str {
    match zone {
        Zone::Own => "own",
        Zone::Synced | Zone::Hidden | Zone::Outside => "synced",
    }
}
