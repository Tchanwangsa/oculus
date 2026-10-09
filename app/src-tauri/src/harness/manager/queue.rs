use std::collections::{HashMap, VecDeque};
use std::sync::atomic::{AtomicU64, Ordering};

use serde::Serialize;

use super::options::SendOptions;

/// A message typed while a turn was running, waiting its own.
#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct QueuedMessage {
    pub id: String,
    pub text: String,
}

fn next_queue_id() -> String {
    static N: AtomicU64 = AtomicU64::new(1);
    format!("q{}", N.fetch_add(1, Ordering::SeqCst))
}

/// Which threads have a turn open, and what is waiting behind each.
///
/// The CLIs mishandle a message sent mid-turn (`claude` silently chains a
/// second turn; `codex` folds it into the running one), so one turn per
/// thread runs and the rest wait here. A pending message has no row until it
/// goes out, which is why it can still be edited or dropped.
#[derive(Default)]
pub struct Queue {
    threads: HashMap<i64, ThreadQueue>,
}

#[derive(Default)]
struct ThreadQueue {
    /// A turn of ours is open. Released by its `TurnFinished` — every bridge
    /// emits exactly one per message it accepts.
    busy: bool,
    pending: VecDeque<(QueuedMessage, SendOptions)>,
}

impl Queue {
    /// Take the thread for a send. False when a turn already has it.
    pub fn try_claim(&mut self, thread_id: i64) -> bool {
        let q = self.threads.entry(thread_id).or_default();
        if q.busy {
            return false;
        }
        q.busy = true;
        true
    }

    /// Fall in behind the turn that has the thread.
    pub fn push(&mut self, thread_id: i64, text: &str, opts: &SendOptions) -> QueuedMessage {
        let msg = QueuedMessage {
            id: next_queue_id(),
            text: text.to_string(),
        };
        self.threads
            .entry(thread_id)
            .or_default()
            .pending
            .push_back((msg.clone(), opts.clone()));
        msg
    }

    /// The turn ended: the next message waiting, if there is one. The thread
    /// stays claimed when one is handed back — it is about to be sent — and
    /// goes idle when nothing is.
    pub fn next(&mut self, thread_id: i64) -> Option<(QueuedMessage, SendOptions)> {
        let q = self.threads.entry(thread_id).or_default();
        match q.pending.pop_front() {
            Some(next) => Some(next),
            None => {
                q.busy = false;
                None
            }
        }
    }

    /// Everything still waiting, dropped — what stop does. Handed back so
    /// the composer can return them to the student.
    pub fn clear(&mut self, thread_id: i64) -> Vec<QueuedMessage> {
        match self.threads.get_mut(&thread_id) {
            Some(q) => q.pending.drain(..).map(|(m, _)| m).collect(),
            None => Vec::new(),
        }
    }

    /// Drop one pending message.
    pub fn remove(&mut self, thread_id: i64, id: &str) -> bool {
        let Some(q) = self.threads.get_mut(&thread_id) else {
            return false;
        };
        let before = q.pending.len();
        q.pending.retain(|(m, _)| m.id != id);
        q.pending.len() != before
    }

    /// Rewrite one that has not gone out yet.
    pub fn edit(&mut self, thread_id: i64, id: &str, text: &str) -> Option<QueuedMessage> {
        let q = self.threads.get_mut(&thread_id)?;
        let (m, _) = q.pending.iter_mut().find(|(m, _)| m.id == id)?;
        m.text = text.to_string();
        Some(m.clone())
    }

    pub fn list(&self, thread_id: i64) -> Vec<QueuedMessage> {
        match self.threads.get(&thread_id) {
            Some(q) => q.pending.iter().map(|(m, _)| m.clone()).collect(),
            None => Vec::new(),
        }
    }

    /// A turn of ours is open on this thread, or something is waiting behind
    /// one. Anything that rewrites the thread's rows has to wait for both.
    pub fn is_busy(&self, thread_id: i64) -> bool {
        self.threads.get(&thread_id).is_some_and(|q| q.busy)
    }

    pub fn forget(&mut self, thread_id: i64) {
        self.threads.remove(&thread_id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_thread_runs_one_turn_and_the_rest_wait_in_order() {
        let mut q = Queue::default();
        let opts = SendOptions::default();
        assert!(q.try_claim(1), "an idle thread is taken by the first send");
        assert!(!q.try_claim(1), "and not by the second");

        let a = q.push(1, "first", &opts);
        let b = q.push(1, "second", &opts);
        assert_eq!(q.list(1).len(), 2);
        // Another thread is not held up by this one.
        assert!(q.try_claim(2));

        assert_eq!(
            q.next(1).map(|(m, _)| m),
            Some(a),
            "in the order they were typed"
        );
        assert!(
            !q.try_claim(1),
            "the thread stays claimed while one is going out"
        );
        assert_eq!(q.next(1).map(|(m, _)| m.text), Some("second".into()));
        assert!(q.next(1).is_none(), "nothing left");
        assert!(q.try_claim(1), "and the thread is free again");
        let _ = b;
    }

    #[test]
    fn stopping_clears_the_queue_and_returns_what_it_held() {
        let mut q = Queue::default();
        let opts = SendOptions::default();
        q.try_claim(7);
        q.push(7, "one", &opts);
        let two = q.push(7, "two", &opts);
        q.push(7, "three", &opts);
        assert!(
            q.remove(7, &two.id),
            "a pending message can be dropped on its own"
        );
        assert_eq!(
            q.clear(7).into_iter().map(|m| m.text).collect::<Vec<_>>(),
            vec!["one".to_string(), "three".to_string()]
        );
        assert!(q.list(7).is_empty());
        // Only the running turn's `TurnFinished` releases the thread.
        assert!(!q.try_claim(7));
        assert!(q.next(7).is_none());
        assert!(q.try_claim(7));
    }
}
