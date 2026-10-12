//! A challenge workspace's zip, unpacked into the files worth keeping.

use std::io::Read;

/// A file bigger than this is left out of the library and named in the log.
pub const MAX_ENTRY_BYTES: u64 = 20 * 1024 * 1024;

/// One file of a workspace, at its path inside the workspace.
pub struct WorkspaceFile {
    /// `/`-separated, never absolute and never climbing out (`enclosed_name`).
    pub path: String,
    pub bytes: Vec<u8>,
}

/// The kept files, in the zip's order, plus one sentence per file left out
/// for its size or an unsafe path. Directories, dotfiles, anything inside a
/// dot-directory and macOS's `__MACOSX` resource forks are dropped silently.
pub fn unpack(zip: &[u8]) -> Result<(Vec<WorkspaceFile>, Vec<String>), String> {
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
            skipped.push(format!("{}: unsafe path, skipped", entry.name()));
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
            skipped.push(format!("{path}: over 20 MB, skipped"));
            continue;
        }
        // The header's size is the zip's claim; the read is capped regardless.
        let mut bytes = Vec::new();
        (&mut entry)
            .take(MAX_ENTRY_BYTES + 1)
            .read_to_end(&mut bytes)
            .map_err(|e| format!("{path}: unreadable in the zip: {e}"))?;
        if bytes.len() as u64 > MAX_ENTRY_BYTES {
            skipped.push(format!("{path}: over 20 MB, skipped"));
            continue;
        }
        files.push(WorkspaceFile { path, bytes });
    }
    Ok((files, skipped))
}
