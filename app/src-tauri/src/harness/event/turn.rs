//! Whose turn an event belongs to. A bridge stamps `TurnStarted` and
//! `TurnFinished` with the turn, and a send hands the consumer the same id
//! before any event of that turn (docs/harness.md, "Events say whose they
//! are").

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

/// A turn's id within its session: Codex's own turn id; for the other
/// bridges the count of turns opened on the session, the CLI's own included.
pub type TurnId = String;

/// Told the turn a send was accepted as, before any event of that turn.
pub type Bound = Arc<dyn Fn(TurnId) + Send + Sync>;

/// For a send nothing tracks: a one-off turn.
pub fn unbound() -> Bound {
    Arc::new(|_| {})
}

/// Turn ordinals for a bridge whose CLI names no turn. A send reserves its
/// turn's before the provider write; a turn the CLI starts itself takes the
/// next as it opens.
#[derive(Default)]
pub struct Turns {
    opened: AtomicU64,
    sent: Mutex<Option<u64>>,
}

impl Turns {
    fn next(&self) -> u64 {
        self.opened.fetch_add(1, Ordering::SeqCst) + 1
    }

    /// A send is about to be written: the turn it will open as.
    pub fn reserve(&self) -> TurnId {
        let turn = self.next();
        *self.sent.lock().unwrap() = Some(turn);
        turn.to_string()
    }

    /// The waiting send's turn, taken by its opening, or by its end when it
    /// never opened.
    pub fn take_sent(&self) -> Option<TurnId> {
        self.sent.lock().unwrap().take().map(|t| t.to_string())
    }

    /// A turn the CLI started itself.
    pub fn own(&self) -> TurnId {
        self.next().to_string()
    }

    /// A turn opened: the waiting send's, else the CLI's own.
    pub fn open(&self) -> TurnId {
        self.take_sent().unwrap_or_else(|| self.own())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_send_and_a_turn_the_cli_starts_never_share_an_ordinal() {
        let turns = Turns::default();
        let sent = turns.reserve();
        assert_eq!(sent, "1");
        // The CLI opens a turn of its own before it reads the message.
        assert_eq!(turns.own(), "2");
        assert_eq!(turns.open(), "1", "the send's turn keeps its ordinal");
        assert_eq!(turns.open(), "3", "nothing waiting: the CLI's own");
        assert_eq!(turns.take_sent(), None);
    }
}
