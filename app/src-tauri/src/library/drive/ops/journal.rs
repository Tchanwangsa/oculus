//! The disk half of a command, undoable: every rename, every folder made or
//! removed and every entry made is recorded, so when the rows cannot follow
//! the disk goes back. Nothing is deleted before the rows commit: an entry a
//! merge replaces is set aside under a dot-name beside it, restored on undo
//! and removed by `commit`.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use crate::library::files::documents::same_entry;

enum Step {
    Renamed { from: PathBuf, to: PathBuf },
    MadeDir(PathBuf),
    RemovedDir(PathBuf),
    SetAside { original: PathBuf, aside: PathBuf },
    Made(PathBuf),
}

#[derive(Default)]
pub struct Journal {
    steps: Vec<Step>,
}

impl Journal {
    /// Rename `from` to `to`, making `to`'s missing folders first. `to` may
    /// be `from` itself under another case (a case-only rename on a
    /// case-insensitive volume); anything else already there refuses with
    /// `AlreadyExists`, checked right before the rename, so two entries
    /// whose names the volume folds together never land on one.
    pub fn rename(&mut self, from: &Path, to: &Path) -> std::io::Result<()> {
        if std::fs::symlink_metadata(to).is_ok() && !same_entry(from, to) {
            let name = to.file_name().unwrap_or_default().to_string_lossy();
            return Err(std::io::Error::new(
                std::io::ErrorKind::AlreadyExists,
                format!("{name} already exists there"),
            ));
        }
        if let Some(parent) = to.parent() {
            self.make_dirs(parent)?;
        }
        std::fs::rename(from, to)?;
        self.steps.push(Step::Renamed {
            from: from.to_path_buf(),
            to: to.to_path_buf(),
        });
        Ok(())
    }

    /// `create_dir_all`, recording each folder it made.
    fn make_dirs(&mut self, dir: &Path) -> std::io::Result<()> {
        let mut missing = Vec::new();
        let mut at = Some(dir);
        while let Some(path) = at.filter(|p| std::fs::symlink_metadata(p).is_err()) {
            missing.push(path.to_path_buf());
            at = path.parent();
        }
        for path in missing.into_iter().rev() {
            std::fs::create_dir(&path)?;
            self.steps.push(Step::MadeDir(path));
        }
        Ok(())
    }

    /// Record an entry this command made, removed again on undo.
    pub fn made(&mut self, path: &Path) {
        self.steps.push(Step::Made(path.to_path_buf()));
    }

    /// Move `path` out of the way to a dot-name beside it.
    fn set_aside(&mut self, path: &Path) -> std::io::Result<()> {
        static SEQUENCE: AtomicU64 = AtomicU64::new(0);
        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        let aside = path.with_file_name(format!(
            ".{name}.drive-aside-{}-{}",
            std::process::id(),
            SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::rename(path, &aside)?;
        self.steps.push(Step::SetAside {
            original: path.to_path_buf(),
            aside,
        });
        Ok(())
    }

    /// Remove an emptied folder, recorded so undo makes it again.
    fn remove_empty_dir(&mut self, dir: &Path) {
        if std::fs::remove_dir(dir).is_ok() {
            self.steps.push(Step::RemovedDir(dir.to_path_buf()));
        }
    }

    /// The rows followed: what was set aside goes for good.
    pub fn commit(self) {
        for step in self.steps {
            if let Step::SetAside { aside, .. } = step {
                let removed = if aside.is_dir() {
                    std::fs::remove_dir_all(&aside)
                } else {
                    std::fs::remove_file(&aside)
                };
                if let Err(e) = removed {
                    elog!("[oculus] drive: could not remove {}: {e}", aside.display());
                }
            }
        }
    }

    /// Put the disk back as it was, newest first. Best effort: a failure is
    /// logged, since the command is already failing with its own error.
    pub fn undo(self) {
        for step in self.steps.into_iter().rev() {
            let undone = match &step {
                Step::Renamed { from, to } => from
                    .parent()
                    .map_or(Ok(()), std::fs::create_dir_all)
                    .and_then(|()| std::fs::rename(to, from)),
                Step::MadeDir(dir) => std::fs::remove_dir(dir),
                Step::RemovedDir(dir) => std::fs::create_dir_all(dir),
                Step::SetAside { original, aside } => std::fs::rename(aside, original),
                Step::Made(path) if path.is_dir() => std::fs::remove_dir_all(path),
                Step::Made(path) => std::fs::remove_file(path),
            };
            if let Err(e) = undone {
                elog!("[oculus] drive: could not undo a step: {e}");
            }
        }
    }
}

/// Move `from` onto `to`, merging into a folder already there (a stale
/// mirror of parse records): files replace theirs (set aside until commit),
/// folders merge. When `to` is `from` under another case, a plain rename.
pub fn merge_move(journal: &mut Journal, from: &Path, to: &Path) -> std::io::Result<()> {
    let to_meta = match std::fs::symlink_metadata(to) {
        Err(_) => return journal.rename(from, to),
        Ok(_) if same_entry(from, to) => return journal.rename(from, to),
        Ok(meta) => meta,
    };
    if std::fs::symlink_metadata(from)?.is_dir() && to_meta.is_dir() {
        for child in std::fs::read_dir(from)?.flatten() {
            merge_move(journal, &child.path(), &to.join(child.file_name()))?;
        }
        journal.remove_empty_dir(from);
        return Ok(());
    }
    journal.set_aside(to)?;
    journal.rename(from, to)
}
