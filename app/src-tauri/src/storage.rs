//! Disk usage of the app's data directory, and Start over, which empties it.
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
    crate::blocking::run(|| {
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
}

// ── Start over ───────────────────────────────────────────────────────────────
//
// Clears the library so the app can be pointed at another university. The
// running app only signs out and leaves a marker, then restarts; the next
// launch deletes the files before any thread or database handle exists, so
// nothing is still writing into what is being deleted.

/// Kept: each cloud's quota and spend guard belong to the student's API
/// accounts, not to the library.
const KEPT_ON_START_OVER: &[&str] = &["mineru-usage.json", "voyage-usage.json"];

fn start_over_marker(data_dir: &Path) -> std::path::PathBuf {
    data_dir.join("start-over")
}

/// Empty the data dir if Start over asked for it, keeping
/// [`KEPT_ON_START_OVER`]. The first thing the app and the CLI do. The marker
/// goes last, so an interrupted run finishes on the next launch.
pub fn finish_start_over(data_dir: &Path) {
    let marker = start_over_marker(data_dir);
    if !marker.exists() {
        return;
    }
    let Ok(entries) = std::fs::read_dir(data_dir) else {
        return;
    };
    let mut failed = false;
    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name();
        if path == marker || KEPT_ON_START_OVER.iter().any(|k| name == *k) {
            continue;
        }
        let removed = match entry.file_type() {
            Ok(t) if t.is_dir() => std::fs::remove_dir_all(&path),
            _ => std::fs::remove_file(&path),
        };
        if let Err(e) = removed {
            eprintln!(
                "[oculus] start over: could not remove {}: {e}",
                path.display()
            );
            failed = true;
        }
    }
    if !failed {
        std::fs::remove_file(&marker).ok();
        eprintln!("[oculus] start over: library cleared");
    }
}

/// Sign out everywhere, forget the university's credentials and the
/// keep-alive agent, then restart into [`finish_start_over`]. The frontend
/// clears its own library-scoped storage first.
#[tauri::command]
pub async fn storage_start_over(app: tauri::AppHandle) -> Result<(), String> {
    use tauri::Manager;

    let dir = crate::paths::data_dir();
    std::fs::write(start_over_marker(&dir), b"1").map_err(|e| e.to_string())?;
    if let Some(cancel) = app.try_state::<crate::scrape::ScrapeCancel>() {
        cancel.0.store(true, std::sync::atomic::Ordering::SeqCst);
    }
    crate::harness::app::shutdown(&app);
    crate::auth::end_session(&app).await?;
    crate::okta::clear_credentials().ok();
    crate::keepalive::keepalive_disable().await.ok();
    app.restart()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::Scratch;

    #[test]
    fn start_over_empties_the_library_but_keeps_cloud_usage() {
        let dir = Scratch::new("start-over");
        std::fs::create_dir_all(dir.join("courses/COMP30026/files")).unwrap();
        std::fs::write(dir.join("courses/COMP30026/files/a.pdf"), b"%PDF").unwrap();
        std::fs::write(dir.join("oculus.db"), b"").unwrap();
        std::fs::write(dir.join("institution.json"), b"{}").unwrap();
        std::fs::write(dir.join("voyage-usage.json"), b"{}").unwrap();
        std::fs::write(dir.join("mineru-usage.json"), b"{}").unwrap();

        finish_start_over(&dir);
        assert!(
            dir.join("oculus.db").exists(),
            "nothing happens without the marker"
        );

        std::fs::write(start_over_marker(&dir), b"1").unwrap();
        finish_start_over(&dir);
        let mut left: Vec<_> = std::fs::read_dir(&*dir)
            .unwrap()
            .map(|e| e.unwrap().file_name().into_string().unwrap())
            .collect();
        left.sort();
        assert_eq!(left, ["mineru-usage.json", "voyage-usage.json"]);
    }
}
