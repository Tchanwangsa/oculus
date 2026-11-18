//! `delete_entries` and its confirmation's list. There is no trash: a file
//! goes with its converted PDF, parse artifacts (beside it and in the
//! derived folder), row, pages, versions, task links and page pictures.
//! Rows go first, so a share or parse that needs the row cannot start on a
//! file about to vanish; the job fence is checked before either.

use std::path::Path;

use sqlx::SqlitePool;

use super::busy::fenced;
use super::companions::derived_dir;
use super::error::DriveResult;
use super::guard::{files_under, own_entry, Entry};
use super::moves::blocking;
use super::rows;
use crate::library::paths::{artifact_slots, notebook_pages_dir};

/// Every file a delete of `rels` would remove that the student sees (what
/// Drive lists: no parse artifacts, converted PDFs, pictures folders), sorted.
pub fn delete_preview_at(data_dir: &Path, rels: &[String]) -> DriveResult<Vec<String>> {
    let mut out = Vec::new();
    for rel in rels {
        let entry = own_entry(data_dir, rel)?;
        if entry.dir {
            let listing = crate::library::drive::listing::list_folder_at(data_dir, rel, true, &[])
                .map_err(super::error::DriveError::io)?;
            out.extend(listing.files.into_iter().map(|f| f.relative_path));
        } else {
            out.push(entry.rel);
        }
    }
    out.sort();
    out.dedup();
    Ok(out)
}

/// Delete `rels` (files and folders of the student's own).
pub async fn delete_entries_at(
    data_dir: &Path,
    pool: &SqlitePool,
    rels: &[String],
) -> DriveResult<()> {
    let entries = rels
        .iter()
        .map(|rel| own_entry(data_dir, rel))
        .collect::<DriveResult<Vec<Entry>>>()?;
    let files: Vec<String> = entries.iter().flat_map(files_under).collect();
    let dir = data_dir.to_path_buf();
    let ((), held) = blocking(move || fenced(&dir, &files, || Ok(()))).await?;
    let keys: Vec<(String, bool)> = entries.iter().map(|e| (e.rel.clone(), e.dir)).collect();
    let ids = rows::delete_rows(pool, &keys).await?;
    let dir = data_dir.to_path_buf();
    blocking(move || remove(&dir, &entries, &ids)).await?;
    drop(held);
    Ok(())
}

fn remove(data_dir: &Path, entries: &[Entry], ids: &[i64]) -> DriveResult<()> {
    for entry in entries {
        if entry.dir {
            if let Some(mirror) = derived_dir(data_dir, &entry.rel) {
                std::fs::remove_dir_all(mirror).ok();
            }
            std::fs::remove_dir_all(&entry.path)?;
            continue;
        }
        // The artifacts first: a leftover would be served as the parse of
        // whatever lands on this name next.
        for (_, artifact) in artifact_slots(data_dir, &entry.rel) {
            let _ = if artifact.is_dir() {
                std::fs::remove_dir_all(artifact)
            } else {
                std::fs::remove_file(artifact)
            };
        }
        std::fs::remove_file(&entry.path)?;
    }
    for id in ids {
        if notebook_pages_dir(data_dir, *id).is_dir() {
            if let Err(e) = crate::library::files::whiteboards::snapshots::remove_at(data_dir, *id)
            {
                elog!("[oculus] drive: page pictures of file {id}: {e}");
            }
        }
    }
    Ok(())
}
