//! The renders in flight, at most one per video folder, since two would
//! write the same segments, cache index and output. Each is keyed by its
//! canonical folder and holds the cancel flag its render checks; `Running`
//! removes it when dropped, on every way a render ends.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use super::quality::Quality;

/// The app's renders.
pub static RENDERS: Renders = Renders::new();

pub struct Renders {
    running: Mutex<BTreeMap<PathBuf, Entry>>,
}

struct Entry {
    quality: Quality,
    cancel: Arc<AtomicBool>,
}

impl Renders {
    pub const fn new() -> Self {
        Self {
            running: Mutex::new(BTreeMap::new()),
        }
    }

    /// Claim `dir` for a render, or refuse naming the render that holds it.
    pub fn start(&self, dir: &Path, quality: Quality) -> Result<Running<'_>, String> {
        let key = std::fs::canonicalize(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
        let mut running = self.lock();
        if let Some(entry) = running.get(&key) {
            return Err(format!(
                "a {} render of {} is already running; wait for it or cancel it",
                entry.quality.name(),
                key.display()
            ));
        }
        let cancel = Arc::new(AtomicBool::new(false));
        running.insert(
            key.clone(),
            Entry {
                quality,
                cancel: cancel.clone(),
            },
        );
        Ok(Running {
            renders: self,
            key,
            cancel,
        })
    }

    /// Ask the render of `dir` to stop; false when none runs.
    pub fn cancel(&self, dir: &Path) -> bool {
        let Ok(key) = std::fs::canonicalize(dir) else {
            return false;
        };
        match self.lock().get(&key) {
            Some(entry) => {
                entry.cancel.store(true, Ordering::Relaxed);
                true
            }
            None => false,
        }
    }

    /// A render that panicked while holding the lock left the map whole.
    fn lock(&self) -> MutexGuard<'_, BTreeMap<PathBuf, Entry>> {
        self.running.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

impl Default for Renders {
    fn default() -> Self {
        Self::new()
    }
}

/// A claimed folder; dropping it frees the folder.
pub struct Running<'a> {
    renders: &'a Renders,
    key: PathBuf,
    cancel: Arc<AtomicBool>,
}

impl Running<'_> {
    /// The flag `render` checks, set by `Renders::cancel`.
    pub fn cancel_flag(&self) -> &AtomicBool {
        &self.cancel
    }
}

impl Drop for Running<'_> {
    fn drop(&mut self) {
        let mut running = self.renders.lock();
        if running
            .get(&self.key)
            .is_some_and(|entry| Arc::ptr_eq(&entry.cancel, &self.cancel))
        {
            running.remove(&self.key);
        }
    }
}
