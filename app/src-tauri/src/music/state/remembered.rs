//! What Oculus last played, as the state keeps it (`last_played/`): read
//! back at connect, resumed after a drop, played by Play with nothing
//! playing anywhere, forgotten at sign-out.

use std::time::Instant;

use tauri::AppHandle;

use super::{change, now_ms, with};
use crate::music::last_played::LastPlayed;

/// Whether run `id`, connected as `account`, should read the row: nothing is
/// remembered and no track is shown. What another account left is forgotten
/// first, and so is the track it showed.
pub(in crate::music) fn needs_recall(app: &AppHandle, id: u64, account: &str) -> bool {
    let mut needs = false;
    change(app, |inner| {
        if !inner.is_run(id) {
            return false;
        }
        if inner.memory.account().is_some_and(|a| a != account) {
            inner.memory.forget();
            if !inner.snapshot.active {
                inner.snapshot.track = None;
            }
        }
        needs = inner.memory.is_empty() && inner.snapshot.track.is_none();
        true
    });
    needs
}

/// Shows the row read back, paused where it stood, while nothing else is
/// shown.
pub(in crate::music) fn recall(app: &AppHandle, id: u64, row: LastPlayed) {
    change(app, |inner| {
        let shown = inner.snapshot.active || inner.snapshot.track.is_some();
        if !inner.is_run(id) || shown || !inner.memory.is_empty() {
            return false;
        }
        let s = &mut inner.snapshot;
        s.track = Some(row.track.clone());
        s.position_ms = row.position_ms;
        s.position_at_ms = now_ms();
        s.playing = false;
        s.context_uri = row.context_uri.clone();
        s.shuffle = row.shuffle;
        s.set_repeat(row.repeat_context, row.repeat_track);
        inner.memory.recall(row, Instant::now());
        true
    });
}

/// Sign-out: nothing is remembered in memory; the row stays for the account.
pub(in crate::music) fn forget_last(app: &AppHandle) {
    with(app, |inner| inner.memory.forget());
}

/// What the player shows when nothing plays anywhere, for Play to load.
pub(in crate::music) fn remembered(app: &AppHandle) -> Option<LastPlayed> {
    with(app, |inner| {
        let account = inner.snapshot.account.as_deref()?;
        inner
            .memory
            .shown(Instant::now())
            .filter(|row| row.account == account)
    })
}

/// Whether run `id` has a resume waiting on its reconnect.
pub(in crate::music) fn resume_pending(app: &AppHandle, id: u64) -> bool {
    with(app, |inner| {
        inner.is_run(id) && inner.memory.resume_pending()
    })
}

/// A pause while a resume waits makes it load paused.
pub(in crate::music) fn hold_resume_paused(app: &AppHandle) {
    with(app, |inner| inner.memory.hold_paused());
}

/// Run `id`'s waiting resume, taken: what to load and whether it plays.
pub(in crate::music) fn take_resume(app: &AppHandle, id: u64) -> Option<(LastPlayed, bool)> {
    with(app, |inner| {
        if !inner.is_run(id) {
            return None;
        }
        inner.memory.take_resume(Instant::now())
    })
}
