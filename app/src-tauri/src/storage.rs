//! Disk usage of the app's data directory.
//!
//! One walk returns every file with its size; the frontend owns the grouping
//! (by file type, by subject) so new views need no Rust changes. Free/total
//! space for the volume the data dir sits on comes from statvfs.

use std::path::Path;

use serde::Serialize;

#[derive(Serialize)]
pub struct StorageFile {
    /// Path relative to the data dir, `/`-separated.
    pub path: String,
    pub bytes: u64,
}

#[derive(Serialize)]
pub struct StorageReport {
    pub data_dir: String,
    pub total_bytes: u64,
    pub disk_free_bytes: u64,
    pub disk_total_bytes: u64,
    pub files: Vec<StorageFile>,
}

fn walk(root: &Path, dir: &Path, out: &mut Vec<StorageFile>, total: &mut u64) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        // symlink_metadata so a stray link can't loop the walk or double-count.
        let Ok(meta) = entry.path().symlink_metadata() else {
            continue;
        };
        if meta.is_dir() {
            walk(root, &path, out, total);
        } else if meta.is_file() {
            let bytes = meta.len();
            *total += bytes;
            let rel = path
                .strip_prefix(root)
                .map(|p| p.to_string_lossy().replace('\\', "/"))
                .unwrap_or_else(|_| path.to_string_lossy().into_owned());
            out.push(StorageFile { path: rel, bytes });
        }
    }
}

#[cfg(unix)]
fn disk_space(path: &Path) -> (u64, u64) {
    use std::os::unix::ffi::OsStrExt;
    let Ok(c) = std::ffi::CString::new(path.as_os_str().as_bytes()) else {
        return (0, 0);
    };
    let mut s: libc::statvfs = unsafe { std::mem::zeroed() };
    if unsafe { libc::statvfs(c.as_ptr(), &mut s) } != 0 {
        return (0, 0);
    }
    let frsize = s.f_frsize as u64;
    (s.f_bavail as u64 * frsize, s.f_blocks as u64 * frsize)
}

#[cfg(not(unix))]
fn disk_space(_path: &Path) -> (u64, u64) {
    (0, 0)
}

#[tauri::command]
pub async fn storage_report() -> Result<StorageReport, String> {
    tauri::async_runtime::spawn_blocking(|| {
        let root = crate::paths::data_dir();
        let mut files = Vec::new();
        let mut total = 0u64;
        walk(&root, &root, &mut files, &mut total);
        files.sort_by(|a, b| b.bytes.cmp(&a.bytes));
        let (disk_free_bytes, disk_total_bytes) = disk_space(&root);
        Ok(StorageReport {
            data_dir: root.to_string_lossy().into_owned(),
            total_bytes: total,
            disk_free_bytes,
            disk_total_bytes,
            files,
        })
    })
    .await
    .map_err(|e| e.to_string())?
}
