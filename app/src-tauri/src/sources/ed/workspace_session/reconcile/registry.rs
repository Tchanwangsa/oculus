//! Which challenges have a session open in this process, and which the
//! lesson sync is reconciling: the two never reconcile one workspace at
//! once. The sync leaves an open challenge to its session; a session that
//! connects while the sync is on its challenge waits for it.

use std::collections::BTreeSet;
use std::sync::{Condvar, Mutex, MutexGuard};

struct Marks {
    open: BTreeSet<i64>,
    syncing: BTreeSet<i64>,
}

static MARKS: Mutex<Marks> = Mutex::new(Marks {
    open: BTreeSet::new(),
    syncing: BTreeSet::new(),
});
static CHANGED: Condvar = Condvar::new();

fn marks() -> MutexGuard<'static, Marks> {
    MARKS.lock().unwrap_or_else(|e| e.into_inner())
}

pub fn opened(challenge_id: i64) {
    marks().open.insert(challenge_id);
}

pub fn closed(challenge_id: i64) {
    marks().open.remove(&challenge_id);
}

/// The sync's claim on a challenge, released when dropped.
pub struct Syncing(i64);

impl Drop for Syncing {
    fn drop(&mut self) {
        marks().syncing.remove(&self.0);
        CHANGED.notify_all();
    }
}

/// Claim a challenge for the sync; `None` while a session has it open.
pub fn begin_sync(challenge_id: i64) -> Option<Syncing> {
    let mut marks = marks();
    if marks.open.contains(&challenge_id) {
        return None;
    }
    marks.syncing.insert(challenge_id);
    Some(Syncing(challenge_id))
}

/// Block while the sync holds the challenge.
pub fn wait_for_sync(challenge_id: i64) {
    let mut marks = marks();
    while marks.syncing.contains(&challenge_id) {
        marks = CHANGED.wait(marks).unwrap_or_else(|e| e.into_inner());
    }
}
