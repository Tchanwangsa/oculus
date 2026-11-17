//! Moving a staged download to where the student chose, and throwing one
//! away. Each runs on its own thread: an Office file's PDF conversion takes
//! seconds, and the download hook is on the main thread.

use std::io::ErrorKind;
use std::path::Path;

use tauri::AppHandle;

use super::state::{Effect, Target};
use super::{broadcast, SavedTo};
use crate::library::drive::ops::guard::own_target;
use crate::library::files::uploads::import_into;
use crate::shell::browser::with_state;

/// Runs each effect off the caller's thread; a delivery reports back through
/// `browser-downloads`.
pub(super) fn run(app: &AppHandle, effects: Vec<Effect>) {
    for effect in effects {
        let app = app.clone();
        let spawned = std::thread::Builder::new()
            .name("browser-download".into())
            .spawn(move || apply(&app, effect));
        if let Err(e) = spawned {
            elog!("[oculus] browser: could not start a download move: {e}");
        }
    }
}

fn apply(app: &AppHandle, effect: Effect) {
    match effect {
        Effect::Remove(staged) => remove_staged(&staged),
        Effect::Deliver { id, staged, to } => {
            let result = deliver(&crate::library::paths::data_dir(), &staged, &to);
            match &result {
                Ok(_) => elog!("[oculus] browser: download {id} saved"),
                Err(e) => elog!("[oculus] browser: download {id} not saved: {e}"),
            }
            with_state(app, |s| s.downloads.delivered(id, result));
            broadcast(app);
        }
    }
}

/// Lands `staged` at `to`. The staged copy is gone after a success and
/// untouched after a failure, so another choice can be made.
pub(super) fn deliver(data_dir: &Path, staged: &Path, to: &Target) -> Result<SavedTo, String> {
    match to {
        Target::Oculus {
            subject_code,
            target_dir,
        } => {
            let target =
                own_target(data_dir, subject_code, target_dir.as_deref()).map_err(|e| e.message)?;
            let outcome = import_into(data_dir, &target, &[staged])
                .pop()
                .ok_or("nothing was imported")?;
            if outcome.file.is_none() {
                return Err(outcome
                    .error
                    .unwrap_or_else(|| "it could not be added".into()));
            }
            remove_staged(staged);
            Ok(SavedTo::Oculus {
                subject_code: subject_code.clone(),
                outcome,
            })
        }
        Target::Path { path, may_replace } => {
            move_file(staged, path, *may_replace, |from, to| {
                std::fs::rename(from, to)
            })?;
            remove_staged(staged);
            Ok(SavedTo::Path {
                path: path.to_string_lossy().into_owned(),
            })
        }
    }
}

/// Moves `from` to `to` with `rename`, or copies it across when the two are
/// on different volumes. An existing file at `to` is replaced only when
/// `may_replace` (the save panel asked); never a folder or a link.
pub(super) fn move_file(
    from: &Path,
    to: &Path,
    may_replace: bool,
    rename: impl Fn(&Path, &Path) -> std::io::Result<()>,
) -> Result<(), String> {
    let shown = to
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| to.display().to_string());
    match std::fs::symlink_metadata(to) {
        Ok(meta) if meta.is_dir() || meta.file_type().is_symlink() => {
            return Err(format!("{shown} is not a file that can be replaced"));
        }
        Ok(_) if !may_replace => return Err(format!("{shown} already exists there")),
        Err(e) if e.kind() != ErrorKind::NotFound => return Err(e.to_string()),
        _ => {}
    }
    match rename(from, to) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == ErrorKind::CrossesDevices => copy_across(from, to, &shown),
        Err(e) => Err(format!("could not move it there — {e}")),
    }
}

/// A copy beside `to`, renamed over it once whole, so a half-copied file
/// never shows under the real name; then the original goes.
fn copy_across(from: &Path, to: &Path, shown: &str) -> Result<(), String> {
    let partial = to.with_file_name(format!(".{shown}.oculus-partial"));
    let copied = std::fs::copy(from, &partial).and_then(|_| std::fs::rename(&partial, to));
    if let Err(e) = copied {
        std::fs::remove_file(&partial).ok();
        return Err(format!("could not copy it there — {e}"));
    }
    std::fs::remove_file(from).ok();
    Ok(())
}

/// Deletes the folder holding one staged download. Only a folder directly
/// under `.incoming/` is ever removed.
pub(super) fn remove_staged(staged: &Path) {
    let Some(folder) = staged.parent() else {
        return;
    };
    let under_incoming = folder
        .parent()
        .and_then(|p| p.file_name())
        .is_some_and(|n| n == ".incoming");
    if !under_incoming {
        elog!(
            "[oculus] browser: not removing {}: outside the staging folder",
            folder.display()
        );
        return;
    }
    if let Err(e) = std::fs::remove_dir_all(folder) {
        if e.kind() != ErrorKind::NotFound {
            elog!("[oculus] browser: could not remove a staged download: {e}");
        }
    }
}
