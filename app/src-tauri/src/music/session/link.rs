//! A link while it is up, and its end. librespot marks a closed connection's
//! Session invalid but has no notifier, and the Spirc only checks on its next
//! wake (a push, a player event or a command), so a drop could go unseen
//! until the user acted: the link polls the session every second instead.
//! The same poll sees the Mac wake, and reconnects at once: a socket that
//! died in sleep is otherwise only noticed after up to 80 s awake.

use std::future::Future;
use std::pin::Pin;
use std::time::Duration;

use librespot_core::Session;
use tokio::time::MissedTickBehavior;

use super::policy::Stamp;
use super::{Link, DROP_GRACE};
use crate::music::state;

/// How often an up link checks its session and ticks the last-played row.
const POLL: Duration = Duration::from_secs(1);

/// Why a link stopped being up.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Unlinked {
    /// `stop_run` asked.
    Stopped,
    /// The Spirc's task ended by itself.
    Ended,
    /// librespot marked the session invalid: its connection closed.
    Closed,
    /// The Mac has just woken: a poll tick found more wall-clock time than
    /// awake time since the one before.
    Slept,
}

impl Unlinked {
    pub(super) fn reason(self) -> &'static str {
        match self {
            Unlinked::Stopped => "stopped",
            Unlinked::Ended => "the device's task ended",
            Unlinked::Closed => "the session closed",
            Unlinked::Slept => "the Mac slept",
        }
    }
}

impl Link<'_> {
    /// Waits while the link is up. `music_wake` checks the session at once;
    /// a sleep is seen by the tick after it, whoever is looking.
    pub(super) async fn linked<F: Future<Output = ()>>(
        &mut self,
        session: &Session,
        mut task: Pin<&mut F>,
    ) -> Unlinked {
        let mut poll = tokio::time::interval(POLL);
        poll.set_missed_tick_behavior(MissedTickBehavior::Delay);
        let mut last_tick = Stamp::now();
        loop {
            tokio::select! {
                _ = task.as_mut() => return Unlinked::Ended,
                _ = self.stop.changed() => return Unlinked::Stopped,
                _ = poll.tick() => {
                    if session.is_invalid() {
                        return Unlinked::Closed;
                    }
                    if last_tick.slept() {
                        return Unlinked::Slept;
                    }
                    last_tick = Stamp::now();
                    state::tick(self.app);
                }
                _ = self.wake.notified() => {
                    if session.is_invalid() {
                        return Unlinked::Closed;
                    }
                }
            }
        }
    }

    /// Ends a dropped link: what played is kept for the resume and paused
    /// here so it picks up where the drop left it, the session is closed,
    /// and a Spirc still running is woken to wind down (a `disconnect` that
    /// keeps playback alone, which an inactive Spirc ignores; either way its
    /// loop then sees the closed session), for at most `DROP_GRACE`.
    pub(super) async fn unlink<F: Future<Output = ()>>(
        &mut self,
        session: &Session,
        mut task: Pin<&mut F>,
        why: Unlinked,
    ) {
        let lost = state::lose_link(self.app, self.id);
        if lost.playing {
            self.player.pause();
        }
        if !session.is_invalid() {
            session.shutdown();
        }
        if why == Unlinked::Ended {
            return;
        }
        if let Some(spirc) = &lost.spirc {
            spirc.disconnect(false).ok();
        }
        if tokio::time::timeout(DROP_GRACE, task.as_mut())
            .await
            .is_err()
        {
            elog!("[music] the dropped Spotify device did not wind down within 3 s");
        }
    }
}
