//! What travels with a file when it moves: its converted PDF and its parse
//! artifacts (`paths::artifact_slots`), each to where the same slot lives
//! for the new path. The text and its pictures sit beside the file; the parse
//! record and the page vectors sit in the subject's hidden derived folder,
//! mirroring the file's folder, so a move to another folder or subject moves
//! them there too and the file stays parsed and embedded.

use std::path::{Path, PathBuf};

use crate::library::layout::{self, DerivedKind, Zone};
use crate::library::paths::{artifact_at, artifact_slots, slash_path, ArtifactSlot};

pub struct Companion {
    pub slot: ArtifactSlot,
    pub from: PathBuf,
    /// `None` when the new name has no such artifact (a `.docx` renamed to
    /// `.txt`): the old one is removed instead.
    pub to: Option<PathBuf>,
    /// In app state, where a leftover at `to` is stale and replaced; one
    /// beside the file is something the student sees, and a clash.
    pub hidden: bool,
}

/// The companions of the file at `from_rel` that exist, for its move to
/// `to_rel`.
pub fn companions(data_dir: &Path, from_rel: &str, to_rel: &str) -> Vec<Companion> {
    artifact_slots(data_dir, from_rel)
        .into_iter()
        .filter(|(_, from)| std::fs::symlink_metadata(from).is_ok())
        .map(|(slot, from)| {
            let to = artifact_at(data_dir, to_rel, slot);
            let hidden = is_hidden(data_dir, &from);
            Companion {
                slot,
                from,
                to,
                hidden,
            }
        })
        .collect()
}

fn is_hidden(data_dir: &Path, path: &Path) -> bool {
    path.strip_prefix(data_dir)
        .is_ok_and(|rel| layout::zone_of(&slash_path(rel)) == Zone::Hidden)
}

/// The hidden folder mirroring `folder_rel` that holds its files' parse
/// records, when the layout keeps them apart from the files.
pub fn derived_dir(data_dir: &Path, folder_rel: &str) -> Option<PathBuf> {
    let folder = data_dir.join(folder_rel);
    let probe = layout::derived(&folder.join("probe.pdf"), DerivedKind::Pages);
    let dir = probe.parent()?.to_path_buf();
    (dir != folder).then_some(dir)
}

/// The link prefix a parsed file's markdown carries for its pictures (the
/// pictures folder's name), for the PDF at `pdf`.
pub fn images_prefix(pdf: &Path) -> String {
    layout::derived(pdf, DerivedKind::Images)
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default()
}
