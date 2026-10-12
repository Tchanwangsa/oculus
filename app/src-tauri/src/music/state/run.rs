//! The session task's side of the state: `music_wake`'s nudge, whether a
//! run may start by itself, a dropped link, and the periodic tick.

use std::time::Instant;

use librespot_connect::Spirc;
use tauri::AppHandle;

use super::{change, with};

/// Nudges the running session task to check its connection (`session/`).
/// False when no task runs.
pub(in crate::music) fn wake(app: &AppHandle) -> bool {
    with(app, |inner| match inner.run.as_ref() {
        Some(run) => {
            run.wake.notify_one();
            true
        }
        None => false,
    })
}

/// Whether a session may start without the user asking: none runs, no login
/// is needed or waiting, and the user has not disconnected.
pub(in crate::music) fn may_start(app: &AppHandle) -> bool {
    with(app, |inner| {
        inner.run.is_none() && inner.login.is_none() && !inner.held && !inner.snapshot.needs_login
    })
}

/// The user disconnected: nothing reconnects by itself until they connect.
pub(in crate::music) fn hold(app: &AppHandle) {
    with(app, |inner| inner.held = true);
}

/// A dropped link's leftovers for the session task.
pub(in crate::music) struct Lost {
    /// The device handle, to wind its task down.
    pub spirc: Option<Spirc>,
    /// The resume plays: the task pauses the player until it does.
    pub playing: bool,
}

/// Run `id`'s connection dropped: what Oculus played is kept for a resume
/// and written, the snapshot says it is reconnecting (keeping the track),
/// and the device handle comes back to be wound down.
pub(in crate::music) fn lose_link(app: &AppHandle, id: u64) -> Lost {
    let mut lost = Lost {
        spirc: None,
        playing: false,
    };
    change(app, |inner| {
        if !inner.is_run(id) {
            return false;
        }
        if let Some(row) = inner.memory.lose(inner.snapshot.active, Instant::now()) {
            inner.save(row);
        }
        lost.playing = inner.memory.resume_plays();
        lost.spirc = inner.drop_link();
        let s = &mut inner.snapshot;
        s.connected = false;
        s.connecting = true;
        s.reconnecting = true;
        s.error = None;
        s.set_unlinked();
        true
    });
    lost
}

/// The session task's periodic tick: the last-played row while it plays.
pub(in crate::music) fn tick(app: &AppHandle) {
    with(app, |inner| {
        if let Some(row) = inner.memory.tick(Instant::now()) {
            inner.save(row);
        }
    });
}
