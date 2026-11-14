//! Where a thread's sessions report, and the number that tells one session
//! from the next.

use crate::harness::event::TurnId;
use crate::harness::Sink;

/// One spawn or attach of a thread's session. Never reused, so an event from
/// a session that was replaced or has ended can be told from the live one's.
pub type Generation = u64;

/// Where one thread's sessions report. The app's goes to the consumer's bus
/// (`app::sinks_for`); a headless run has only its events ([`OneSink`]).
pub trait Sinks: Send + Sync {
    /// The sink the session minted as `generation` keeps for its life.
    fn events(&self, generation: Generation) -> Sink;

    /// A send was accepted as `turn` of that session. Called before any
    /// event of the turn can arrive.
    fn bound(&self, _generation: Generation, _turn: TurnId) {}
}

/// The same sink for every session: a caller with no consumer behind it.
pub struct OneSink(pub Sink);

impl Sinks for OneSink {
    fn events(&self, _: Generation) -> Sink {
        self.0.clone()
    }
}
