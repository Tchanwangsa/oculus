//! Which run each thread's events belong to, and its settlement. Ownership
//! is assigned here, on the one ordered consumer: a persisted question (or a
//! card's continuation) opens its run, and that run owns every later event on
//! the thread until its `TurnFinished` settles it. A session is known by its
//! generation, and one that has been replaced is heard no more.

use std::collections::{HashMap, HashSet};
use std::sync::Mutex;

use sqlx::SqlitePool;
use tokio::runtime::Runtime;

use crate::harness::{store, Generation, HarnessEvent, Queue};

/// Who sent an event.
#[derive(Clone, Copy, PartialEq, Debug)]
pub(super) enum From {
    /// The app itself: a queued message, a send that failed before its turn.
    App,
    /// A bridge's session.
    Session(Generation),
}

struct Open {
    run_id: i64,
    attempt_id: i64,
    /// The next row's attempt-local ordinal.
    next: u64,
}

#[derive(Default)]
pub(super) struct RunScope {
    open: HashMap<i64, Open>,
    /// Threads in a turn the provider started itself (unowned for now).
    unprompted: HashSet<i64>,
    /// Threads whose open turn already reported a failed row write.
    failed_writes: HashSet<i64>,
    /// Each thread's newest session.
    sessions: HashMap<i64, Generation>,
}

/// One event's place in a run, for its envelope.
#[derive(Default)]
pub(super) struct Tags {
    pub run_id: Option<i64>,
    pub attempt_id: Option<i64>,
}

impl RunScope {
    /// Whether `generation` is still the thread's session. A newer one
    /// replaces it for good: generations only count up.
    pub fn admit(&mut self, thread_id: i64, generation: Generation) -> bool {
        let current = self.sessions.entry(thread_id).or_default();
        if generation < *current {
            return false;
        }
        *current = generation;
        true
    }

    /// A `TurnFinished` with no turn open and no claim on the thread: a
    /// repeat of one already applied.
    pub fn is_stray_terminal(
        &self,
        thread_id: i64,
        ev: &HarnessEvent,
        queue: &Mutex<Queue>,
    ) -> bool {
        matches!(ev, HarnessEvent::TurnFinished { .. })
            && !self.open.contains_key(&thread_id)
            && !self.unprompted.contains(&thread_id)
            && !queue.lock().unwrap().is_busy(thread_id)
    }

    /// Opens the thread's run, or its unowned turn, before `ev` is stored.
    pub fn enter(&mut self, thread_id: i64, ev: &HarnessEvent) -> Tags {
        match ev {
            HarnessEvent::UserMessage { run: Some(run), .. }
            | HarnessEvent::Continued { run: Some(run), .. } => {
                self.failed_writes.remove(&thread_id);
                self.open.insert(
                    thread_id,
                    Open {
                        run_id: run.run_id,
                        attempt_id: run.attempt_id,
                        next: 0,
                    },
                );
            }
            HarnessEvent::UnpromptedTurn { .. } => {
                self.failed_writes.remove(&thread_id);
                self.open.remove(&thread_id);
                self.unprompted.insert(thread_id);
            }
            _ => {}
        }
        self.open
            .get(&thread_id)
            .map(|o| Tags {
                run_id: Some(o.run_id),
                attempt_id: Some(o.attempt_id),
            })
            .unwrap_or_default()
    }

    /// The owner a row made from `ev` would get. The question's own row (or
    /// a continuation's) was written with its run, so it takes none here.
    pub fn owner(&self, thread_id: i64, ev: &HarnessEvent) -> Option<store::Owner> {
        if matches!(
            ev,
            HarnessEvent::UserMessage { .. } | HarnessEvent::Continued { .. }
        ) {
            return None;
        }
        self.open.get(&thread_id).map(|o| store::Owner {
            run_id: o.run_id,
            attempt_id: o.attempt_id,
            entity: format!("weak:{}", o.next),
        })
    }

    /// A row was written under the thread's run: the next takes the next ordinal.
    pub fn written(&mut self, thread_id: i64) {
        if let Some(o) = self.open.get_mut(&thread_id) {
            o.next += 1;
        }
    }

    /// True once per turn: the first row the store could not write.
    pub fn first_storage_failure(&mut self, thread_id: i64) -> bool {
        self.failed_writes.insert(thread_id)
    }

    /// Run-level effects of an already stored event: the binding learns its
    /// session; `TurnFinished` settles the run and closes the scope. Returns
    /// the settlement's events, each with its row, to emit before the terminal.
    pub fn after(
        &mut self,
        rt: &Runtime,
        pool: &SqlitePool,
        thread_id: i64,
        ev: &HarnessEvent,
    ) -> Vec<(HarnessEvent, Option<i64>)> {
        if matches!(ev, HarnessEvent::TurnFinished { .. }) {
            self.unprompted.remove(&thread_id);
        }
        let Some(open) = self.open.get(&thread_id) else {
            return Vec::new();
        };
        let mut out = Vec::new();
        let stored = match ev {
            HarnessEvent::SessionStarted {
                provider_session_id,
                ..
            } => rt.block_on(store::bind_session(pool, open.run_id, provider_session_id)),
            // Uncommitted answer text survives a restart (`store::recover`).
            HarnessEvent::AssistantDelta { text } => {
                rt.block_on(store::append_draft(pool, open.run_id, text))
            }
            HarnessEvent::AssistantMessage { .. } => {
                rt.block_on(store::clear_draft(pool, open.run_id))
            }
            HarnessEvent::TurnFinished { status } => {
                let settled = rt.block_on(store::settle_run(
                    pool,
                    open.run_id,
                    open.attempt_id,
                    status,
                ));
                self.open.remove(&thread_id);
                settled.map(|s| {
                    if let Some(s) = s {
                        if let Some((id, text)) = s.notice {
                            out.push((HarnessEvent::DeliveryNotice { text }, Some(id)));
                        }
                        out.push((
                            HarnessEvent::RunSettled {
                                answer_item_id: s.answer_item_id,
                                work_item_ids: s.work_item_ids,
                                delivery: s.delivery.into(),
                                status: s.status.into(),
                                started_at: s.started_at,
                                ended_at: s.ended_at,
                            },
                            None,
                        ));
                    }
                })
            }
            _ => Ok(()),
        };
        if let Err(e) = stored {
            elog!("[oculus] harness run {thread_id}: {e}");
        }
        out
    }
}
