//! Component and theme libraries: the starter set compiled into the crate,
//! seeding it into a library folder, and the one write path every library
//! edit takes, which snapshots the old text into `.history` first.
//!
//! A library folder holds `components/<Name>.tsx`, `themes/<name>.json` and
//! `.history/` (snapshots named `<stem>.<unix-ms>.<ext>`, never pruned).

mod starter;

pub use starter::{STARTER_COMPONENTS, STARTER_THEMES, Starter};

use crate::error::{Result, StudioError};
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

/// Copies every starter file that is missing from `root`, never touching a
/// file that exists. With `reset`, each starter file whose text differs is
/// written over the library's, snapshotting the old text first. Returns the
/// paths written, relative to `root`.
pub fn seed(root: &Path, reset: bool) -> Result<Vec<String>> {
    let files = STARTER_COMPONENTS
        .iter()
        .map(|(name, text)| (format!("components/{name}.tsx"), *text))
        .chain(
            STARTER_THEMES
                .iter()
                .map(|(name, text)| (format!("themes/{name}.json"), *text)),
        );
    seed_files(root, files, reset)
}

fn seed_files<'a>(
    root: &Path,
    files: impl Iterator<Item = (String, &'a str)>,
    reset: bool,
) -> Result<Vec<String>> {
    let mut written = Vec::new();
    for (rel, text) in files {
        let path = root.join(&rel);
        let exists = path.try_exists().map_err(|e| StudioError::io(&path, e))?;
        if exists {
            if !reset {
                continue;
            }
            let current = std::fs::read_to_string(&path).map_err(|e| StudioError::io(&path, e))?;
            if current == text {
                continue;
            }
        }
        write(root, &rel, text)?;
        written.push(rel);
    }
    Ok(written)
}

/// Writes `text` to `root/rel` (`components/<Name>.tsx` or
/// `themes/<name>.json`). A file that exists is first copied to
/// `root/.history`, so the previous text is always recoverable. The new
/// text lands through a temporary file and a rename.
pub fn write(root: &Path, rel: &str, text: &str) -> Result<()> {
    let valid = matches!(rel.split_once('/'), Some(("components" | "themes", file))
        if !file.is_empty() && !file.contains('/') && !file.starts_with('.'));
    if !valid {
        return Err(StudioError::Render(format!(
            "library path `{rel}` must be components/<Name>.tsx or themes/<name>.json"
        )));
    }
    let path = root.join(rel);
    let parent = path.parent().unwrap_or(root);
    std::fs::create_dir_all(parent).map_err(|e| StudioError::io(parent, e))?;
    if path.try_exists().map_err(|e| StudioError::io(&path, e))? {
        snapshot(root, rel, &path)?;
    }
    let tmp = path.with_extension("tmp");
    std::fs::write(&tmp, text).map_err(|e| StudioError::io(&tmp, e))?;
    std::fs::rename(&tmp, &path).map_err(|e| StudioError::io(&path, e))
}

/// Copies `path` to `root/.history/<stem>.<unix-ms>.<ext>`, moving the
/// millisecond on past any snapshot already there.
fn snapshot(root: &Path, rel: &str, path: &Path) -> Result<()> {
    let file = rel.rsplit('/').next().unwrap_or(rel);
    let (stem, ext) = file.rsplit_once('.').unwrap_or((file, ""));
    let dir = root.join(".history");
    std::fs::create_dir_all(&dir).map_err(|e| StudioError::io(&dir, e))?;
    let mut ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|e| StudioError::Render(format!("the clock is before 1970: {e}")))?
        .as_millis();
    loop {
        let dest = dir.join(format!("{stem}.{ms}.{ext}"));
        if !dest.try_exists().map_err(|e| StudioError::io(&dest, e))? {
            std::fs::copy(path, &dest).map_err(|e| StudioError::io(&dest, e))?;
            return Ok(());
        }
        ms += 1;
    }
}

#[cfg(test)]
mod tests;
