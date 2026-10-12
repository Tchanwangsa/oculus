//! Component and theme libraries: the starter set compiled into the crate,
//! seeding it into a library folder, and the one write path every library
//! edit takes, which snapshots the old text into `.history` first.
//!
//! A library folder holds `components/<Name>.tsx`, `themes/<name>.json`,
//! `.history/` (snapshots named `<stem>.<unix-ms>.<ext>`, never pruned) and
//! `.seeded` (the starter files already offered, one path a line).

mod starter;

pub use starter::{STARTER_COMPONENTS, STARTER_THEMES, Starter};

use crate::error::{Result, StudioError};
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

/// Copies each starter file `root` has never been offered, once: a file
/// listed in `.seeded` is not copied again, so a starter the user deleted
/// stays deleted, while one added to a later build still arrives. An existing
/// file is never touched. With `reset`, every starter file whose text differs
/// (or is missing) is written, snapshotting the old text first. Returns the
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
    let record = root.join(SEEDED);
    let mut seeded: Vec<String> = match std::fs::read_to_string(&record) {
        Ok(s) => s.lines().map(str::to_owned).collect(),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Vec::new(),
        Err(e) => return Err(StudioError::io(&record, e)),
    };
    let before = seeded.len();
    let mut written = Vec::new();
    for (rel, text) in files {
        let offered = seeded.contains(&rel);
        if offered && !reset {
            continue;
        }
        let path = root.join(&rel);
        let exists = path.try_exists().map_err(|e| StudioError::io(&path, e))?;
        let write_it = if exists {
            reset && std::fs::read_to_string(&path).map_err(|e| StudioError::io(&path, e))? != text
        } else {
            true
        };
        if write_it {
            write(root, &rel, text)?;
            written.push(rel.clone());
        }
        if !offered {
            seeded.push(rel);
        }
    }
    if seeded.len() != before {
        std::fs::create_dir_all(root).map_err(|e| StudioError::io(root, e))?;
        let tmp = record.with_extension("tmp");
        std::fs::write(&tmp, seeded.join("\n") + "\n").map_err(|e| StudioError::io(&tmp, e))?;
        std::fs::rename(&tmp, &record).map_err(|e| StudioError::io(&record, e))?;
    }
    Ok(written)
}

/// The record of starter files already offered to a library.
const SEEDED: &str = ".seeded";

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
