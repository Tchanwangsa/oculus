use std::collections::HashMap;

/// Which threads have a turn open.
///
/// The CLIs mishandle a message sent mid-turn (`claude` silently chains a
/// second turn; `codex` folds it into the running one), so one turn per
/// thread runs and the rest wait as queued runs in the database
/// (`store/queue.rs`). A turn the provider starts itself
/// ([`UnpromptedTurn`](crate::harness::HarnessEvent::UnpromptedTurn)) holds
/// the thread the same way. Nothing here survives a restart, and nothing
/// needs to: no turn does either.
#[derive(Default)]
pub struct Queue {
    /// Turns holding each thread. Each is released by its `TurnFinished`:
    /// every bridge emits exactly one per message it accepts and per turn
    /// the provider starts itself. Two at once is a send racing such a turn.
    open: HashMap<i64, u32>,
}

impl Queue {
    /// Take the thread for a send. False when a turn already has it.
    pub fn try_claim(&mut self, thread_id: i64) -> bool {
        let open = self.open.entry(thread_id).or_default();
        if *open > 0 {
            return false;
        }
        *open = 1;
        true
    }

    /// The provider started a turn of its own: the thread is held until its
    /// `TurnFinished`, whatever else holds it.
    pub fn hold(&mut self, thread_id: i64) {
        *self.open.entry(thread_id).or_default() += 1;
    }

    /// A turn ended. True once no other turn holds the thread, which is
    /// when the next waiting run may start.
    pub fn release(&mut self, thread_id: i64) -> bool {
        let open = self.open.entry(thread_id).or_default();
        *open = open.saturating_sub(1);
        *open == 0
    }

    /// A turn is open on this thread. Anything that rewrites the thread's
    /// rows has to wait for it.
    pub fn is_busy(&self, thread_id: i64) -> bool {
        self.open.get(&thread_id).is_some_and(|open| *open > 0)
    }

    pub fn forget(&mut self, thread_id: i64) {
        self.open.remove(&thread_id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_thread_runs_one_turn_at_a_time() {
        let mut q = Queue::default();
        assert!(q.try_claim(1), "an idle thread is taken by the first send");
        assert!(!q.try_claim(1), "and not by the second");
        // Another thread is not held up by this one.
        assert!(q.try_claim(2));
        assert!(q.release(1), "its turn ended");
        assert!(q.try_claim(1), "and the thread is free again");
    }

    #[test]
    fn a_turn_the_provider_starts_holds_an_idle_thread() {
        let mut q = Queue::default();
        q.hold(3);
        assert!(q.is_busy(3));
        assert!(
            !q.try_claim(3),
            "a send waits behind the provider's own turn"
        );
        assert!(q.release(3));
        assert!(!q.is_busy(3));
    }

    #[test]
    fn a_hold_racing_a_claim_releases_after_both_turns() {
        let mut q = Queue::default();
        assert!(q.try_claim(4));
        q.hold(4);
        assert!(!q.release(4), "one turn still holds the thread");
        assert!(!q.try_claim(4));
        assert!(q.release(4));
        assert!(q.try_claim(4));
    }

    #[test]
    fn a_forgotten_thread_is_free() {
        let mut q = Queue::default();
        q.hold(5);
        q.forget(5);
        assert!(!q.is_busy(5));
        assert!(q.release(5), "a late end of a forgotten turn holds nothing");
    }
}
