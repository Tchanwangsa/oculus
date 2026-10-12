//! The in-memory side: what Oculus played last and where it stood, a resume
//! waiting on a reconnect, and when the row is due a write. Positions advance
//! by `Instant`, which stops while the Mac sleeps, as the player does.

use std::time::{Duration, Instant};

use super::LastPlayed;
use crate::music::state::Snapshot;

/// How often the row is written while a track plays; track changes, pauses,
/// drops and stops write at once.
pub(in crate::music) const WRITE_EVERY: Duration = Duration::from_secs(15);
/// How long before a drop Oculus may have stopped being the device playing
/// and still be resumed: a Spirc that ends by itself says it disconnected
/// before its task ends, which can take its requests' time on a dead network.
pub(in crate::music) const LEFT_RECENTLY: Duration = Duration::from_secs(30);

#[derive(Debug, Default)]
pub(in crate::music) struct Memory {
    last: Option<Last>,
    /// A restore waiting on the reconnect after a drop: whether it plays.
    resume: Option<bool>,
    written: Option<Written>,
    /// The last change found Oculus the device playing a track.
    following: bool,
    /// When Oculus last stopped being the device playing, and whether it was
    /// playing then.
    left: Option<(Instant, bool)>,
}

#[derive(Clone, Debug)]
struct Last {
    /// `position_ms` was true at `at`.
    row: LastPlayed,
    playing: bool,
    at: Instant,
    /// The snapshot's `position_at_ms` it was taken from, to tell a new
    /// position from an old one.
    stamp: u64,
}

/// What a write is keyed on: any change writes at once.
#[derive(Clone, Debug, PartialEq)]
struct Key {
    track: String,
    context: Option<String>,
    playing: bool,
    shuffle: bool,
    repeat_context: bool,
    repeat_track: bool,
}

#[derive(Debug)]
struct Written {
    key: Key,
    at: Instant,
}

/// The position `elapsed` after `position_ms`: moved on only while playing,
/// and never past the track's end (a duration of 0 is unknown).
pub(in crate::music) fn advanced(
    position_ms: u32,
    duration_ms: u32,
    playing: bool,
    elapsed: Duration,
) -> u32 {
    if !playing {
        return position_ms;
    }
    let at = (position_ms as u64).saturating_add(elapsed.as_millis() as u64);
    let at = if duration_ms > 0 {
        at.min(duration_ms as u64)
    } else {
        at
    };
    at.min(u32::MAX as u64) as u32
}

impl Last {
    fn position_at(&self, now: Instant) -> u32 {
        advanced(
            self.row.position_ms,
            self.row.track.duration_ms,
            self.playing,
            now.saturating_duration_since(self.at),
        )
    }

    fn freeze(&mut self, now: Instant) {
        self.row.position_ms = self.position_at(now);
        self.playing = false;
        self.at = now;
    }

    fn key(&self) -> Key {
        Key {
            track: self.row.track.uri.clone(),
            context: self.row.context_uri.clone(),
            playing: self.playing,
            shuffle: self.row.shuffle,
            repeat_context: self.row.repeat_context,
            repeat_track: self.row.repeat_track,
        }
    }

    /// The row with its position as of `now`.
    fn row_at(&self, now: Instant) -> LastPlayed {
        LastPlayed {
            position_ms: self.position_at(now),
            ..self.row.clone()
        }
    }
}

impl Memory {
    /// Follows the snapshot after a change: while Oculus is the device
    /// playing a track it keeps what plays; once it is not, what played
    /// stops where it was. The row to write, when one is due.
    pub(in crate::music) fn follow(&mut self, s: &Snapshot, now: Instant) -> Option<LastPlayed> {
        let (true, Some(track), Some(account)) = (s.active, &s.track, &s.account) else {
            if std::mem::take(&mut self.following) {
                self.left = self.last.as_ref().map(|l| (now, l.playing));
            }
            return self.freeze(now);
        };
        self.following = true;
        self.left = None;
        // An unchanged stamp is an old position: moved on for the same track,
        // and the start for a new one, whose first position is still to come.
        let position_ms = match &self.last {
            Some(last) if last.stamp == s.position_at_ms => {
                if last.row.track.uri == track.uri {
                    last.position_at(now)
                } else {
                    0
                }
            }
            _ => s.position_ms,
        };
        self.last = Some(Last {
            row: LastPlayed {
                account: account.clone(),
                context_uri: s.context_uri.clone(),
                track: track.clone(),
                position_ms,
                saved_at_ms: 0,
                shuffle: s.shuffle,
                repeat_context: s.repeat_context,
                repeat_track: s.repeat && !s.repeat_context,
            },
            playing: s.playing,
            at: now,
            stamp: s.position_at_ms,
        });
        self.due(now, false)
    }

    /// The periodic write while a track plays.
    pub(in crate::music) fn tick(&mut self, now: Instant) -> Option<LastPlayed> {
        self.due(now, false)
    }

    /// The connection dropped: a resume is kept when Oculus was the device
    /// playing (`active`, or until `LEFT_RECENTLY` before), playing again
    /// only if it was playing. What played stops where it was, and is
    /// written.
    pub(in crate::music) fn lose(&mut self, active: bool, now: Instant) -> Option<LastPlayed> {
        let left = self.left.take();
        self.resume = None;
        let last = self.last.as_mut()?;
        self.resume = if active {
            Some(last.playing)
        } else {
            left.filter(|(at, _)| now.saturating_duration_since(*at) <= LEFT_RECENTLY)
                .map(|(_, playing)| playing)
        };
        last.freeze(now);
        self.due(now, true)
    }

    /// The waiting resume plays.
    pub(in crate::music) fn resume_plays(&self) -> bool {
        self.resume == Some(true)
    }

    /// The session stopped: no resume, and the row is written.
    pub(in crate::music) fn stop(&mut self, now: Instant) -> Option<LastPlayed> {
        self.resume = None;
        self.last.as_mut()?.freeze(now);
        self.due(now, true)
    }

    /// The row read back at connect, shown paused where it stood.
    pub(in crate::music) fn recall(&mut self, row: LastPlayed, now: Instant) {
        let last = Last {
            row,
            playing: false,
            at: now,
            stamp: 0,
        };
        self.written = Some(Written {
            key: last.key(),
            at: now,
        });
        self.last = Some(last);
    }

    /// Sign-out, or another account: nothing is remembered.
    pub(in crate::music) fn forget(&mut self) {
        *self = Self::default();
    }

    pub(in crate::music) fn is_empty(&self) -> bool {
        self.last.is_none()
    }

    pub(in crate::music) fn account(&self) -> Option<&str> {
        self.last.as_ref().map(|l| l.row.account.as_str())
    }

    /// What was played, where it stands `now`.
    pub(in crate::music) fn shown(&self, now: Instant) -> Option<LastPlayed> {
        self.last.as_ref().map(|l| l.row_at(now))
    }

    pub(in crate::music) fn resume_pending(&self) -> bool {
        self.resume.is_some()
    }

    /// A pause while a resume waits: the resume loads paused.
    pub(in crate::music) fn hold_paused(&mut self) {
        if let Some(play) = self.resume.as_mut() {
            *play = false;
        }
    }

    /// The waiting resume, taken: what to load and whether it plays.
    pub(in crate::music) fn take_resume(&mut self, now: Instant) -> Option<(LastPlayed, bool)> {
        let play = self.resume.take()?;
        Some((self.shown(now)?, play))
    }

    fn freeze(&mut self, now: Instant) -> Option<LastPlayed> {
        let last = self.last.as_mut().filter(|l| l.playing)?;
        last.freeze(now);
        self.due(now, false)
    }

    /// The row to write when `force`d, when what plays changed since the
    /// last write, or every `WRITE_EVERY` while it plays.
    fn due(&mut self, now: Instant, force: bool) -> Option<LastPlayed> {
        let last = self.last.as_ref()?;
        let key = last.key();
        let due = force
            || match &self.written {
                None => true,
                Some(w) => {
                    w.key != key
                        || (last.playing && now.saturating_duration_since(w.at) >= WRITE_EVERY)
                }
            };
        if !due {
            return None;
        }
        let row = last.row_at(now);
        self.written = Some(Written { key, at: now });
        Some(row)
    }
}
