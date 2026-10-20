//! The canvas authority: the single writer of a shared canvas file. Desktop
//! panes and paired devices submit content transactions; the authority applies
//! each against the canonical document, persists it through the canvas file
//! guard, bumps the revision and fans the change out. A session starts when
//! sharing starts and ends on close, delete or app exit; outside one, canvases
//! save through the ordinary `write_canvas` path. See docs/drawing-canvas.md.

pub(crate) mod commands;
mod disk;
pub mod error;
pub mod events;
pub mod format;
pub mod model;
mod receipts;
pub mod rows;
pub mod session;
pub mod transaction;

#[cfg(test)]
mod tests;

use crate::library::files::ImportedFile;
use error::AuthorityError;
use events::{Phase, Subscribers};
use rows::FileRows;
use session::{Ack, Hub, Op, Resolution, Session, Snapshot};
use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, Mutex, OnceLock};

/// Live sessions by `files.id`. The registry lock is only ever taken before a
/// session's lock (in `open`), never while one is held.
pub struct Authority {
    sessions: Mutex<HashMap<i64, Arc<Session>>>,
    subscribers: Subscribers,
    rows: Arc<dyn FileRows>,
}

/// The app's authority, recording file rows in the shared database.
pub fn global() -> &'static Authority {
    static AUTHORITY: OnceLock<Authority> = OnceLock::new();
    AUTHORITY.get_or_init(|| Authority::new(Arc::new(rows::DbRows)))
}

impl Authority {
    pub fn new(rows: Arc<dyn FileRows>) -> Self {
        Self {
            sessions: Mutex::new(HashMap::new()),
            subscribers: Subscribers::default(),
            rows,
        }
    }

    pub fn subscribers(&self) -> &Subscribers {
        &self.subscribers
    }

    fn hub(&self) -> Hub<'_> {
        Hub {
            events: &self.subscribers,
            rows: self.rows.as_ref(),
        }
    }

    fn registry(&self) -> std::sync::MutexGuard<'_, HashMap<i64, Arc<Session>>> {
        self.sessions.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn session(&self, file_id: i64) -> Result<Arc<Session>, AuthorityError> {
        self.registry()
            .get(&file_id)
            .cloned()
            .ok_or_else(AuthorityError::ended)
    }

    fn forget(&self, file_id: i64, session: &Arc<Session>) {
        let mut sessions = self.registry();
        if sessions
            .get(&file_id)
            .is_some_and(|kept| Arc::ptr_eq(kept, session))
        {
            sessions.remove(&file_id);
        }
    }

    /// Run a whole-file mutation only when no live authority owns this path.
    /// Holding the registry through the mutation also fences a concurrent open.
    pub(super) fn unshared<T>(
        &self,
        root: &Path,
        rel: &str,
        mutation: impl FnOnce() -> Result<T, String>,
    ) -> Result<T, String> {
        let root = root.canonicalize().map_err(|error| error.to_string())?;
        let sessions = self.registry();
        if sessions
            .values()
            .any(|session| session.lock().owns(&root, rel))
        {
            return Err(
                "This canvas is shared. Save, rename or delete it through its shared session."
                    .into(),
            );
        }
        mutation()
    }

    /// Start sharing `rel` from `text`, the desktop's flushed document, which
    /// must equal the file on disk. A file that is already shared answers with
    /// its live snapshot instead.
    pub fn open(
        &self,
        file_id: i64,
        root: &Path,
        rel: &str,
        text: &str,
    ) -> Result<Snapshot, AuthorityError> {
        let root = root
            .canonicalize()
            .map_err(|error| AuthorityError::io(error.to_string()))?;
        let mut sessions = self.registry();
        if let Some(existing) = sessions.get(&file_id) {
            let state = existing.lock();
            if state.phase() != Phase::Ended {
                if !state.owns(&root, rel) {
                    return Err(AuthorityError::invalid(
                        "This canvas is shared under another path.",
                    ));
                }
                return state.snapshot();
            }
        }
        if sessions
            .values()
            .any(|session| session.lock().owns(&root, rel))
        {
            return Err(AuthorityError::invalid(
                "This canvas already has a shared session under another file id.",
            ));
        }
        let session = Arc::new(Session::open(file_id, &root, rel, text)?);
        let snapshot = session.lock().snapshot()?;
        sessions.insert(file_id, session);
        Ok(snapshot)
    }

    pub fn commit(&self, file_id: i64, op: Op) -> Result<Ack, AuthorityError> {
        self.session(file_id)?.lock().commit(op, self.hub())
    }

    pub fn snapshot(&self, file_id: i64) -> Result<Snapshot, AuthorityError> {
        self.session(file_id)?.lock().snapshot()
    }

    pub fn resolve_external(
        &self,
        file_id: i64,
        choice: Resolution,
    ) -> Result<Snapshot, AuthorityError> {
        self.session(file_id)?.lock().resolve(choice, self.hub())
    }

    pub fn rename(&self, file_id: i64, title: &str) -> Result<ImportedFile, AuthorityError> {
        self.session(file_id)?.lock().rename(title, self.hub())
    }

    pub fn delete(&self, file_id: i64) -> Result<(), AuthorityError> {
        let session = self.session(file_id)?;
        session.lock().delete(self.hub())?;
        self.forget(file_id, &session);
        Ok(())
    }

    /// End sharing; closing a file that is not shared is a no-op.
    pub fn close(&self, file_id: i64) {
        let Ok(session) = self.session(file_id) else {
            return;
        };
        session.lock().close(self.hub());
        self.forget(file_id, &session);
    }

    /// End every session, as on app exit.
    pub fn close_all(&self) {
        let ids: Vec<i64> = self.registry().keys().copied().collect();
        for file_id in ids {
            self.close(file_id);
        }
    }
}
