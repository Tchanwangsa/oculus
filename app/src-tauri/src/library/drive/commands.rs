//! The browser's commands: a folder's listing, and a notebook's cover.

use std::path::Path;

use super::entries::FolderListing;
use super::listing::list_folder_at;
use super::recordings::{subject_recordings, Recording};
use crate::library::layout;
use crate::library::paths::{notebook_pages_dir, read_notebook_pages};

const PNG_MAGIC: &[u8] = b"\x89PNG\r\n\x1a\n";

/// One folder of the library, data-dir-relative (`""` is the Drive root, the
/// subjects). `recursive` lists everything below it, flat: the browser's
/// filtered and searched view.
#[tauri::command]
pub async fn list_folder(rel: String, recursive: Option<bool>) -> Result<FolderListing, String> {
    let data_dir = crate::library::paths::data_dir();
    let recursive = recursive.unwrap_or(false);
    let recordings = if recursive {
        Vec::new()
    } else {
        recordings_for(&data_dir, &rel).await
    };
    crate::runtime::blocking::run(move || list_folder_at(&data_dir, &rel, recursive, &recordings))
        .await
}

/// The subject's lectures, when `rel` is inside one. A listing without them
/// is still right about everything else, so a database error is logged, not
/// returned.
async fn recordings_for(data_dir: &Path, rel: &str) -> Vec<Recording> {
    let probe = format!("{rel}/x");
    let Some((folder, _)) = layout::split_course(&probe) else {
        return Vec::new();
    };
    let pool = match crate::db::store::open_pool().await {
        Ok(pool) => pool,
        Err(e) => {
            elog!("[oculus] list_folder: no database for recordings: {e}");
            return Vec::new();
        }
    };
    let found = subject_recordings(&pool, data_dir, folder).await;
    pool.close().await;
    found.unwrap_or_else(|e| {
        elog!("[oculus] list_folder: reading recordings of {folder}: {e}");
        Vec::new()
    })
}

/// A notebook's first page as PNG bytes, from the pictures the editor writes
/// for chat agents (`whiteboards::snapshots`); empty until one is written.
#[tauri::command]
pub async fn drive_notebook_cover(file_id: i64) -> Result<tauri::ipc::Response, String> {
    crate::runtime::blocking::run(move || {
        Ok(notebook_cover(&crate::library::paths::data_dir(), file_id).unwrap_or_default())
    })
    .await
    .map(tauri::ipc::Response::new)
}

pub(super) fn notebook_cover(data_dir: &Path, file_id: i64) -> Option<Vec<u8>> {
    if file_id <= 0 {
        return None;
    }
    let manifest = read_notebook_pages(data_dir, file_id)?;
    let first = manifest.pages.iter().min_by_key(|p| p.page)?;
    if first.image.contains(['/', '\\']) || first.image.starts_with('.') {
        return None;
    }
    let bytes = std::fs::read(notebook_pages_dir(data_dir, file_id).join(&first.image)).ok()?;
    bytes.starts_with(PNG_MAGIC).then_some(bytes)
}
