use std::collections::HashMap;

/// Threads in a turn their CLI started itself
/// ([`UnpromptedTurn`](crate::harness::HarnessEvent::UnpromptedTurn)).
///
/// The CLIs mishandle a message sent mid-turn (`claude` silently chains a
/// second turn; `codex` folds it into the running one), so one turn per
/// thread runs and the rest wait as queued runs (`store/queue.rs`). A run's
/// own turn is a row, which is what holds the thread; a turn the CLI starts
/// has no run and no row, so it is counted here until its `TurnFinished`.
/// Kept apart from the student's hold on the queue (`queue_held`), so
/// resuming the queue cannot release a message into a live turn. Nothing
/// here survives a restart, and nothing needs to: no turn does either.
#[derive(Default)]
pub struct OwnTurns {
    open: HashMap<i64, u32>,
}

impl OwnTurns {
    /// The provider started a turn of its own on this thread.
    pub fn hold(&mut self, thread_id: i64) {
        *self.open.entry(thread_id).or_default() += 1;
    }

    /// One of them ended.
    pub fn release(&mut self, thread_id: i64) {
        if let Some(open) = self.open.get_mut(&thread_id) {
            *open = open.saturating_sub(1);
            if *open == 0 {
                self.open.remove(&thread_id);
            }
        }
    }

    /// A turn of the provider's own is open on this thread.
    pub fn is_held(&self, thread_id: i64) -> bool {
        self.open.contains_key(&thread_id)
    }

    pub fn forget(&mut self, thread_id: i64) {
        self.open.remove(&thread_id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_turn_the_provider_starts_holds_its_thread_until_it_ends() {
        let mut turns = OwnTurns::default();
        assert!(!turns.is_held(3));
        turns.hold(3);
        assert!(turns.is_held(3));
        assert!(!turns.is_held(4), "another thread is not held up by it");
        turns.release(3);
        assert!(!turns.is_held(3));
        turns.release(3);
        assert!(!turns.is_held(3), "a repeated end holds nothing");
    }

    #[test]
    fn two_turns_hold_until_both_end_and_a_forgotten_thread_is_free() {
        let mut turns = OwnTurns::default();
        turns.hold(5);
        turns.hold(5);
        turns.release(5);
        assert!(turns.is_held(5));
        turns.forget(5);
        assert!(!turns.is_held(5));
    }
}
