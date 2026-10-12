//! Workspace paths: the guard every read, write and delete goes through, and
//! the file list, read from disk.

use std::path::{Path, PathBuf};

/// Where workspace file `rel` sits under `root`, the slide's workspace
/// folder. Refused: an empty or absolute path, `.`/`..` or an empty
/// component, a hidden one (the library keeps no dotfiles of a workspace,
/// `workspace::unpack`), `\`, `:` or NUL, and any path whose deepest existing
/// part resolves outside `root` through a symlink.
pub fn resolve(root: &Path, rel: &str) -> Result<PathBuf, String> {
    let refused = || format!("{rel} is not a path inside the workspace");
    if rel.is_empty() || rel.starts_with('/') || rel.contains(['\\', ':', '\0']) {
        return Err(refused());
    }
    let parts: Vec<&str> = rel.split('/').collect();
    if parts.iter().any(|p| p.is_empty() || p.starts_with('.')) {
        return Err(refused());
    }
    let real_root = root
        .canonicalize()
        .map_err(|_| "the workspace folder is missing from the library".to_string())?;
    let target = parts
        .iter()
        .fold(root.to_path_buf(), |p, part| p.join(part));
    let mut existing = target.as_path();
    while existing.symlink_metadata().is_err() {
        existing = existing.parent().ok_or_else(refused)?;
    }
    let real = existing.canonicalize().map_err(|_| refused())?;
    if !real.starts_with(&real_root) {
        return Err(refused());
    }
    Ok(target)
}

/// Every file under `root`, workspace-relative and sorted. Hidden entries
/// and symlinks are left out, as the guard refuses them.
pub fn list(root: &Path) -> Vec<String> {
    let mut files = Vec::new();
    walk(root, "", &mut files);
    files.sort();
    files
}

fn walk(dir: &Path, prefix: &str, files: &mut Vec<String>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        if name.starts_with('.') {
            continue;
        }
        let Ok(kind) = entry.file_type() else {
            continue;
        };
        let rel = format!("{prefix}{name}");
        if kind.is_dir() {
            walk(&entry.path(), &format!("{rel}/"), files);
        } else if kind.is_file() {
            files.push(rel);
        }
    }
}
