//! The disk half of a command, undoable: every rename and every entry made is
//! recorded, so when the rows cannot follow the disk goes back.

use std::path::{Path, PathBuf};

#[derive(Default)]
pub struct Journal {
    renames: Vec<(PathBuf, PathBuf)>,
    made: Vec<PathBuf>,
}

impl Journal {
    /// Rename `from` to `to`, making `to`'s folders first.
    pub fn rename(&mut self, from: &Path, to: &Path) -> std::io::Result<()> {
        if let Some(parent) = to.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::rename(from, to)?;
        self.renames.push((from.to_path_buf(), to.to_path_buf()));
        Ok(())
    }

    /// Record an entry this command made, removed again on undo.
    pub fn made(&mut self, path: &Path) {
        self.made.push(path.to_path_buf());
    }

    /// Put the disk back as it was, newest first. Best effort: a failure is
    /// logged, since the command is already failing with its own error.
    pub fn undo(self) {
        for (from, to) in self.renames.into_iter().rev() {
            if let Err(e) = std::fs::rename(&to, &from) {
                elog!(
                    "[oculus] drive: could not put {} back at {}: {e}",
                    to.display(),
                    from.display()
                );
            }
        }
        for path in self.made.into_iter().rev() {
            let removed = if path.is_dir() {
                std::fs::remove_dir_all(&path)
            } else {
                std::fs::remove_file(&path)
            };
            if let Err(e) = removed {
                elog!("[oculus] drive: could not remove {}: {e}", path.display());
            }
        }
    }
}

/// Move `from` onto `to`, merging into a folder already there (a stale
/// mirror of parse records): files replace theirs, folders merge.
pub fn merge_move(journal: &mut Journal, from: &Path, to: &Path) -> std::io::Result<()> {
    let to_meta = std::fs::symlink_metadata(to);
    let from_dir = std::fs::symlink_metadata(from)?.is_dir();
    match to_meta {
        Err(_) => journal.rename(from, to),
        Ok(meta) if from_dir && meta.is_dir() => {
            for child in std::fs::read_dir(from)?.flatten() {
                merge_move(journal, &child.path(), &to.join(child.file_name()))?;
            }
            std::fs::remove_dir(from).ok();
            Ok(())
        }
        Ok(meta) => {
            if meta.is_dir() {
                std::fs::remove_dir_all(to)?;
            } else {
                std::fs::remove_file(to)?;
            }
            journal.rename(from, to)
        }
    }
}
