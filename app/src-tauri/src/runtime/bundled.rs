//! The native helpers shipped beside the app (`bundle.externalBin`), and where
//! a bundled or a dev build finds one.

use std::path::PathBuf;

/// Where `stem` may be, in order: beside the running executable (Tauri copies
/// externalBin there, for the app and the CLI alike), the resource dir, then
/// the dev copy `app/scripts` writes as `binaries/<stem>-<target-triple>`.
/// Whether a candidate exists or runs is the caller's to check.
pub(crate) fn candidates(stem: &str, resource_dir: Option<PathBuf>) -> Vec<PathBuf> {
    let name = format!("{stem}{}", std::env::consts::EXE_SUFFIX);
    let mut candidates: Vec<PathBuf> = Vec::new();

    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            candidates.push(dir.join(&name));
        }
    }
    if let Some(res) = resource_dir {
        candidates.push(res.join(&name));
    }

    let dev_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("binaries");
    let prefix = format!("{stem}-");
    if let Ok(entries) = std::fs::read_dir(&dev_dir) {
        for entry in entries.flatten() {
            let file = entry.file_name();
            let file = file.to_string_lossy();
            if file.starts_with(&prefix) && !file.ends_with(".part") {
                candidates.push(entry.path());
            }
        }
    }
    candidates
}

/// The first of [`candidates`] that is a file.
pub(crate) fn find(stem: &str, resource_dir: Option<PathBuf>) -> Option<PathBuf> {
    candidates(stem, resource_dir)
        .into_iter()
        .find(|p| p.is_file())
}
