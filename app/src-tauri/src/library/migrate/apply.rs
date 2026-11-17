//! Carrying out the plan's steps, each logged as done as it lands. Every
//! step can run again: a rename whose source is gone and whose target is
//! there landed before a crash. Moving is `rename` only; a library split over
//! two volumes is refused rather than copied.

use std::io;
use std::path::Path;

use super::journal::{DoneLog, Op, Plan};
use super::plan::{holds_no_file, FINDER_FILE};

/// Test seams: stop after this many renames (a crash), or fail one with the
/// error a rename across volumes gets.
#[derive(Clone, Debug, Default)]
pub struct Hooks {
    pub stop_after: Option<usize>,
    pub cross_device_at: Option<usize>,
}

impl Hooks {
    /// The env var a debug build reads to stop after N renames, so a run on a
    /// copied library can be cut short and resumed. Release builds ignore it.
    pub const STOP_AFTER_ENV: &str = "OCULUS_LIBRARY_MOVE_STOP_AFTER";

    pub fn from_env() -> Self {
        if !cfg!(debug_assertions) {
            return Self::default();
        }
        Self {
            stop_after: std::env::var(Self::STOP_AFTER_ENV)
                .ok()
                .and_then(|v| v.parse().ok()),
            cross_device_at: None,
        }
    }
}

/// What stopped a run before every step was done.
pub const STOPPED: &str = "the library move stopped early (test hook); run again to finish it";

pub fn execute(data_dir: &Path, plan: &mut Plan, hooks: &Hooks) -> Result<(), String> {
    let mut log = DoneLog::open(data_dir)?;
    let mut renamed = 0;
    for index in 0..plan.steps.len() {
        if plan.steps[index].done {
            continue;
        }
        let op = plan.steps[index].op.clone();
        if let Op::Rename { from, to } = &op {
            if hooks.stop_after == Some(renamed) {
                return Err(STOPPED.to_string());
            }
            let fake = (hooks.cross_device_at == Some(renamed))
                .then(|| io::Error::from(io::ErrorKind::CrossesDevices));
            rename(data_dir, from, to, fake)?;
            renamed += 1;
        } else {
            remove(data_dir, &op)?;
        }
        log.record(index)?;
        plan.steps[index].done = true;
    }
    Ok(())
}

fn rename(data_dir: &Path, from: &str, to: &str, fake: Option<io::Error>) -> Result<(), String> {
    let (source, target) = (data_dir.join(from), data_dir.join(to));
    let source_there = std::fs::symlink_metadata(&source).is_ok();
    let target_there = std::fs::symlink_metadata(&target).is_ok();
    match (source_there, target_there) {
        (false, true) => return Ok(()),
        (false, false) => {
            elog!("[oculus] library move: {from} is gone, so not moved to {to}");
            return Ok(());
        }
        (true, true) if !same_entry(&source, &target) => {
            return Err(format!(
                "cannot move {from} to {to}: something is already there"
            ));
        }
        _ => {}
    }
    if let Some(dir) = target.parent() {
        std::fs::create_dir_all(dir).map_err(|e| format!("create {}: {e}", dir.display()))?;
    }
    let renamed = match fake {
        Some(e) => Err(e),
        None => std::fs::rename(&source, &target),
    };
    renamed.map_err(|e| {
        if e.kind() == io::ErrorKind::CrossesDevices {
            format!(
                "cannot move {from} to {to}: they are on different volumes. Oculus moves the \
                 library only by renaming, never by copying, so nothing was copied; keep the \
                 whole data folder on one volume and open Oculus again"
            )
        } else {
            format!("cannot move {from} to {to}: {e}")
        }
    })
}

/// True when two spellings name one entry (a case-only rename on a
/// case-insensitive volume).
fn same_entry(a: &Path, b: &Path) -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        match (std::fs::symlink_metadata(a), std::fs::symlink_metadata(b)) {
            (Ok(a), Ok(b)) => a.dev() == b.dev() && a.ino() == b.ino(),
            _ => false,
        }
    }
    #[cfg(not(unix))]
    {
        a.to_string_lossy().to_lowercase() == b.to_string_lossy().to_lowercase()
    }
}

fn remove(data_dir: &Path, op: &Op) -> Result<(), String> {
    let gone = |e: &io::Error| e.kind() == io::ErrorKind::NotFound;
    match op {
        Op::Rename { .. } => unreachable!("renames are carried out above"),
        Op::RemoveFile { path } => {
            let full = data_dir.join(path);
            match std::fs::symlink_metadata(&full) {
                Ok(meta) if meta.is_dir() => {
                    elog!("[oculus] library move: {path} is a folder now, left");
                    Ok(())
                }
                Ok(_) => std::fs::remove_file(&full)
                    .or_else(|e| if gone(&e) { Ok(()) } else { Err(e) })
                    .map_err(|e| format!("remove {path}: {e}")),
                Err(_) => Ok(()),
            }
        }
        Op::RemoveEmptyTree { path } => {
            let full = data_dir.join(path);
            if !full.is_dir() {
                return Ok(());
            }
            if !holds_no_file(&full) {
                elog!("[oculus] library move: {path} holds files now, left");
                return Ok(());
            }
            std::fs::remove_dir_all(&full).map_err(|e| format!("remove {path}: {e}"))
        }
        Op::RemoveDirIfEmpty { path } => {
            let full = data_dir.join(path);
            let Ok(read) = std::fs::read_dir(&full) else {
                return Ok(());
            };
            let names: Vec<String> = read
                .flatten()
                .map(|e| e.file_name().to_string_lossy().into_owned())
                .collect();
            if names.iter().any(|n| n != FINDER_FILE) {
                elog!("[oculus] library move: {path} is not empty, left");
                return Ok(());
            }
            std::fs::remove_file(full.join(FINDER_FILE)).ok();
            std::fs::remove_dir(&full).map_err(|e| format!("remove {path}: {e}"))
        }
    }
}
