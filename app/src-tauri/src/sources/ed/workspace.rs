//! A challenge workspace's zip, unpacked into the files worth keeping; the
//! guard every workspace path goes through; and the prune that makes a
//! slide's workspace folder hold only what Ed's zip has.

use std::collections::HashSet;
use std::io::Read;
use std::path::{Path, PathBuf};

/// A file bigger than this is left out of the library and named in the log.
pub const MAX_ENTRY_BYTES: u64 = 20 * 1024 * 1024;

/// One file of a workspace, at its path inside the workspace.
pub struct WorkspaceFile {
    /// `/`-separated, never absolute and never climbing out (`enclosed_name`).
    pub path: String,
    pub bytes: Vec<u8>,
}

/// A file of the zip left out for its size or an unsafe path; it displays
/// as the log's sentence.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Skipped {
    /// The entry's name, as the zip has it.
    pub path: String,
    pub reason: &'static str,
}

impl std::fmt::Display for Skipped {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}, skipped", self.path, self.reason)
    }
}

/// The kept files, in the zip's order, plus each file left out for its
/// size or an unsafe path. Directories, dotfiles, anything inside a
/// dot-directory and macOS's `__MACOSX` resource forks are dropped silently.
pub fn unpack(zip: &[u8]) -> Result<(Vec<WorkspaceFile>, Vec<Skipped>), String> {
    let mut archive = zip::ZipArchive::new(std::io::Cursor::new(zip))
        .map_err(|e| format!("the workspace download is not a zip: {e}"))?;
    let (mut files, mut skipped) = (Vec::new(), Vec::new());
    for i in 0..archive.len() {
        let mut entry = archive
            .by_index(i)
            .map_err(|e| format!("unreadable zip entry: {e}"))?;
        if entry.is_dir() {
            continue;
        }
        let Some(name) = entry.enclosed_name() else {
            skipped.push(Skipped {
                path: entry.name().to_string(),
                reason: "unsafe path",
            });
            continue;
        };
        let parts: Vec<String> = name
            .components()
            .filter_map(|c| match c {
                std::path::Component::Normal(s) => Some(s.to_string_lossy().into_owned()),
                _ => None,
            })
            .collect();
        if parts.is_empty() || parts.iter().any(|p| p.starts_with('.') || p == "__MACOSX") {
            continue;
        }
        let path = parts.join("/");
        if entry.size() > MAX_ENTRY_BYTES {
            skipped.push(Skipped {
                path,
                reason: "over 20 MB",
            });
            continue;
        }
        // The header's size is the zip's claim; the read is capped regardless.
        let mut bytes = Vec::new();
        (&mut entry)
            .take(MAX_ENTRY_BYTES + 1)
            .read_to_end(&mut bytes)
            .map_err(|e| format!("{path}: unreadable in the zip: {e}"))?;
        if bytes.len() as u64 > MAX_ENTRY_BYTES {
            skipped.push(Skipped {
                path,
                reason: "over 20 MB",
            });
            continue;
        }
        files.push(WorkspaceFile { path, bytes });
    }
    Ok((files, skipped))
}

/// Where workspace file `rel` sits under `root`, the slide's workspace
/// folder. Refused: an empty or absolute path, `.`/`..` or an empty
/// component, a hidden one (the library keeps no dotfiles of a workspace,
/// [`unpack`]), `\`, `:` or NUL, and any path whose deepest existing part
/// resolves outside `root` through a symlink.
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

/// What [`prune`] keeps, all paths absolute.
#[derive(Debug, Default)]
pub struct Keep {
    /// Files, by their exact path: a folder of the same name is not kept.
    pub files: HashSet<PathBuf>,
    /// Paths kept with everything under them: a parse artifact, which may be
    /// a folder of pictures.
    pub prefixes: Vec<PathBuf>,
}

impl Keep {
    fn holds_folder(&self, path: &Path) -> bool {
        self.prefixes.iter().any(|f| path.starts_with(f))
    }

    pub fn holds(&self, path: &Path) -> bool {
        self.files.contains(path) || self.holds_folder(path)
    }
}

/// What [`prune`] did, each path `root`-relative with `/` separators.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Pruned {
    /// The files deleted, sorted.
    pub removed: Vec<String>,
    /// The files that could not be deleted, each with the reason.
    pub failed: Vec<(String, String)>,
}

/// Delete every file under `root` that `keep` does not hold, hidden files
/// and symlinks included (a link is removed, never followed), then every
/// folder left empty, `root` too. A missing `root` prunes nothing.
pub fn prune(root: &Path, keep: &Keep) -> Pruned {
    let mut pruned = Pruned::default();
    prune_dir(root, "", keep, &mut pruned);
    let _ = std::fs::remove_dir(root);
    pruned.removed.sort();
    pruned
}

fn prune_dir(dir: &Path, prefix: &str, keep: &Keep, pruned: &mut Pruned) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let rel = format!("{prefix}{}", entry.file_name().to_string_lossy());
        let Ok(kind) = entry.file_type() else {
            continue;
        };
        // A folder where Ed has a file of that name goes, so the file can land.
        if kind.is_dir() {
            if !keep.holds_folder(&path) {
                prune_dir(&path, &format!("{rel}/"), keep, pruned);
                // Fails, as it should, while anything kept is still inside.
                let _ = std::fs::remove_dir(&path);
            }
            continue;
        }
        if keep.holds(&path) {
            continue;
        }
        match std::fs::remove_file(&path) {
            Ok(()) => pruned.removed.push(rel),
            Err(e) => pruned.failed.push((rel, e.to_string())),
        }
    }
}
