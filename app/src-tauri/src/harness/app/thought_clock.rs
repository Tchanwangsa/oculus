//! Times each reasoning block from its first delta to the finished
//! [`HarnessEvent::Thinking`], for the row's "Thought for 12s". Runs on the
//! consumer loop, which sees every thread's events in order; a bridge that
//! knows the time itself (opencode's part clock) has already set `ms`.

use std::collections::HashMap;
use std::time::Instant;

use crate::harness::HarnessEvent;

#[derive(Default)]
pub(super) struct ThoughtClock {
    since: HashMap<i64, Instant>,
}

impl ThoughtClock {
    pub(super) fn observe(&mut self, thread_id: i64, ev: &mut HarnessEvent) {
        self.observe_at(thread_id, ev, Instant::now());
    }

    fn observe_at(&mut self, thread_id: i64, ev: &mut HarnessEvent, now: Instant) {
        match ev {
            HarnessEvent::ThinkingDelta { .. } => {
                self.since.entry(thread_id).or_insert(now);
            }
            HarnessEvent::Thinking { ms, .. } => {
                let since = self.since.remove(&thread_id);
                if ms.is_none() {
                    *ms = since.map(|t| now.duration_since(t).as_millis() as u64);
                }
            }
            // A block cut off by a stop or a crash must not time the next one.
            HarnessEvent::TurnFinished { .. }
            | HarnessEvent::Exited { .. }
            | HarnessEvent::Rewound { .. } => {
                self.since.remove(&thread_id);
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    fn thought(ms: Option<u64>) -> HarnessEvent {
        HarnessEvent::Thinking {
            text: "x".into(),
            ms,
        }
    }

    fn ms_of(ev: &HarnessEvent) -> Option<u64> {
        match ev {
            HarnessEvent::Thinking { ms, .. } => *ms,
            _ => None,
        }
    }

    #[test]
    fn times_from_the_first_delta() {
        let mut clock = ThoughtClock::default();
        let t0 = Instant::now();
        for at in [0, 500] {
            let mut d = HarnessEvent::ThinkingDelta { text: "a".into() };
            clock.observe_at(1, &mut d, t0 + Duration::from_millis(at));
        }
        let mut ev = thought(None);
        clock.observe_at(1, &mut ev, t0 + Duration::from_millis(1200));
        assert_eq!(ms_of(&ev), Some(1200));

        // The next block starts its own clock; with no deltas it has no time.
        let mut ev = thought(None);
        clock.observe_at(1, &mut ev, t0 + Duration::from_millis(3000));
        assert_eq!(ms_of(&ev), None);
    }

    #[test]
    fn keeps_a_bridge_time_and_threads_apart() {
        let mut clock = ThoughtClock::default();
        let t0 = Instant::now();
        let mut d = HarnessEvent::ThinkingDelta { text: "a".into() };
        clock.observe_at(1, &mut d, t0);
        let mut other = thought(None);
        clock.observe_at(2, &mut other, t0 + Duration::from_millis(50));
        assert_eq!(ms_of(&other), None);
        let mut ev = thought(Some(7));
        clock.observe_at(1, &mut ev, t0 + Duration::from_millis(900));
        assert_eq!(ms_of(&ev), Some(7));
    }

    #[test]
    fn a_finished_turn_drops_an_open_block() {
        let mut clock = ThoughtClock::default();
        let t0 = Instant::now();
        let mut d = HarnessEvent::ThinkingDelta { text: "a".into() };
        clock.observe_at(1, &mut d, t0);
        let mut fin = HarnessEvent::TurnFinished {
            status: "interrupted".into(),
        };
        clock.observe_at(1, &mut fin, t0);
        let mut ev = thought(None);
        clock.observe_at(1, &mut ev, t0 + Duration::from_millis(10));
        assert_eq!(ms_of(&ev), None);
    }
}
