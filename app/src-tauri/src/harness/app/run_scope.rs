//! Which run each thread's events belong to, and which `TurnFinished` may
//! settle it. A persisted question (or a card's continuation) opens its run,
//! the send binds that run to one turn of one session, and only that turn's
//! terminal settles it. A turn the provider starts itself owns no run. A
//! session is known by its generation, and one that has been replaced is
//! heard no more.

use std::collections::{HashMap, HashSet};

use sqlx::SqlitePool;
use tokio::runtime::Runtime;

use crate::harness::event::TurnId;
use crate::harness::{store, Generation, HarnessEvent};

/// Who sent an event.
#[derive(Clone, Copy, PartialEq, Debug)]
pub(super) enum From {
    /// The app itself: a queued message, a send that failed before its turn.
    App,
    /// A bridge's session.
    Session(Generation),
    /// The consumer itself, closing what a session that ended left open.
    Ended(Generation),
}

impl From {
    fn generation(self) -> Option<Generation> {
        match self {
            From::App => None,
            From::Session(g) | From::Ended(g) => Some(g),
        }
    }
}

/// One turn of one session.
type Turn = (Generation, TurnId);

struct Open {
    run_id: i64,
    attempt_id: i64,
    /// The next row's attempt-local ordinal.
    next: u64,
    /// The turn the send was accepted as; none until the bridge says.
    turn: Option<Turn>,
    /// Its terminal was seen; the settle write has not landed yet.
    settling: bool,
}

/// A thread's newest session, the turns of it that are over, and how far
/// its sessions have ended: every generation up to `ended` is gone.
#[derive(Default)]
struct Session {
    generation: Generation,
    finished: HashSet<TurnId>,
    ended: Generation,
}

#[derive(Default)]
pub(super) struct RunScope {
    open: HashMap<i64, Open>,
    /// Threads in a turn the provider started itself, which owns no run:
    /// announced by `UnpromptedTurn`, named by the `TurnStarted` after it.
    own: HashMap<i64, Option<Turn>>,
    /// Threads whose open turn already reported a failed row write.
    failed_writes: HashSet<i64>,
    sessions: HashMap<i64, Session>,
}

/// One event's place in a run, for its envelope.
#[derive(Default, Clone, Copy)]
pub(super) struct Tags {
    pub run_id: Option<i64>,
    pub attempt_id: Option<i64>,
}

/// What a `TurnFinished` ends.
#[derive(Clone, Copy, PartialEq, Debug)]
pub(super) enum Terminal {
    /// The thread's open run: it settles.
    Run,
    /// A turn the provider started itself.
    Own,
    /// No turn and no run: the app closing a send that failed before its
    /// run was written.
    Claim,
    /// Nothing this thread has open: another turn's end, or a repeat.
    Stray,
}

impl RunScope {
    /// The thread's sessions, with `generation` known: a newer one replaces
    /// an older one for good, since generations only count up.
    fn session(&mut self, thread_id: i64, generation: Generation) -> &mut Session {
        let session = self.sessions.entry(thread_id).or_default();
        if generation > session.generation {
            session.generation = generation;
            session.finished.clear();
        }
        session
    }

    /// Whether `ev`, from the session `generation`, is still to be heard:
    /// not once that session has ended or been replaced, except that its
    /// terminal may still settle the run bound to it.
    pub fn admit(&mut self, thread_id: i64, generation: Generation, ev: &HarnessEvent) -> bool {
        let session = self.session(thread_id, generation);
        if generation == session.generation && generation > session.ended {
            return true;
        }
        match (ev, self.open.get(&thread_id)) {
            (
                HarnessEvent::TurnFinished {
                    turn: Some(turn), ..
                },
                Some(open),
            ) => {
                !open.settling
                    && open
                        .turn
                        .as_ref()
                        .is_some_and(|(g, t)| *g == generation && t == turn)
            }
            _ => false,
        }
    }

    /// A send on the thread's open run was accepted as `turn` of the session
    /// `generation`.
    pub fn bind(&mut self, thread_id: i64, generation: Generation, turn: TurnId) {
        self.session(thread_id, generation);
        if let Some(open) = self.open.get_mut(&thread_id) {
            open.turn.get_or_insert((generation, turn));
        }
    }

    /// The session `generation` has ended: nothing more from it is heard.
    pub fn end(&mut self, thread_id: i64, generation: Generation) {
        let session = self.session(thread_id, generation);
        session.ended = session.ended.max(generation);
    }

    /// The run open on the thread, unless its terminal was already seen.
    pub fn open_run(&self, thread_id: i64) -> Option<i64> {
        self.open
            .get(&thread_id)
            .filter(|open| !open.settling)
            .map(|open| open.run_id)
    }

    /// The run open on the thread, its terminal seen or not: the run whose
    /// stream draft the thread holds.
    pub fn draft_run(&self, thread_id: i64) -> Option<i64> {
        self.open.get(&thread_id).map(|open| open.run_id)
    }

    /// The provider is in a turn of its own on this thread.
    pub fn own_open(&self, thread_id: i64) -> bool {
        self.own.contains_key(&thread_id)
    }

    /// The thread is gone: its newest session is heard no more either.
    pub fn silence(&mut self, thread_id: i64) {
        if let Some(session) = self.sessions.get_mut(&thread_id) {
            session.ended = session.generation;
        }
    }

    /// What the session `generation` leaves open on this thread: the run
    /// bound to one of its turns, and a turn of its own (unnamed when its
    /// `TurnStarted` never came).
    pub fn left_open(
        &self,
        thread_id: i64,
        generation: Generation,
    ) -> (Option<(i64, TurnId)>, Option<Option<TurnId>>) {
        let run = self.open.get(&thread_id).and_then(|open| {
            let (g, turn) = open.turn.as_ref()?;
            (*g == generation && !open.settling).then(|| (open.run_id, turn.clone()))
        });
        let own = self.own.get(&thread_id).and_then(|own| match own {
            Some((g, turn)) => (*g == generation).then(|| Some(turn.clone())),
            None => Some(None),
        });
        (run, own)
    }

    /// The thread is being deleted: whatever was open is nobody's now.
    pub fn forget(&mut self, thread_id: i64) {
        self.open.remove(&thread_id);
        self.own.remove(&thread_id);
        self.failed_writes.remove(&thread_id);
    }

    /// What a `TurnFinished` naming `turn` ends on this thread.
    pub fn terminal(&self, thread_id: i64, from: From, turn: Option<&TurnId>) -> Terminal {
        let own = self.own.get(&thread_id);
        let open = match self.open.get(&thread_id) {
            // Already ending: nothing but a turn of the provider's own can
            // end here until the run's settle lands.
            Some(open) if open.settling => {
                let own_turn = from.generation().zip(turn).is_some_and(|(g, t)| {
                    own.is_some_and(|own| own.as_ref().is_some_and(|(og, ot)| *og == g && ot == t))
                });
                return match own_turn {
                    true => Terminal::Own,
                    false => Terminal::Stray,
                };
            }
            open => open,
        };
        // No turn is named by the app, closing a send that failed, or by a
        // bridge that could not tell: whatever is open ends.
        let (Some(generation), Some(turn)) = (from.generation(), turn) else {
            return match (open, own) {
                // A session cut short in a turn of its own it never named.
                (_, Some(_)) if matches!(from, From::Ended(_)) => Terminal::Own,
                (Some(_), _) => Terminal::Run,
                (None, Some(_)) => Terminal::Own,
                // The consumer's own, closing a send whose commit wrote
                // nothing: there is no run to settle, only a turn to end.
                (None, None) if from == From::App => Terminal::Claim,
                (None, None) => Terminal::Stray,
            };
        };
        let is = |held: &Option<Turn>| {
            held.as_ref()
                .is_some_and(|(g, t)| *g == generation && t == turn)
        };
        if open.is_some_and(|o| is(&o.turn)) {
            return Terminal::Run;
        }
        if own.is_some_and(is) {
            return Terminal::Own;
        }
        // A send whose bridge never named its turn is ended by the first
        // terminal of its session that is not a turn already over.
        let over = self
            .sessions
            .get(&thread_id)
            .is_some_and(|s| s.generation != generation || s.finished.contains(turn));
        if open.is_some_and(|o| o.turn.is_none()) && own.is_none() && !over {
            elog!("[oculus] harness thread {thread_id}: turn {turn} ended a run bound to no turn");
            return Terminal::Run;
        }
        Terminal::Stray
    }

    /// The terminal was applied: its turn is over, and a later one naming it
    /// again is a repeat.
    pub fn finished(&mut self, thread_id: i64, from: From, turn: Option<&TurnId>, was: Terminal) {
        if was == Terminal::Own {
            self.own.remove(&thread_id);
        }
        if let (Some(generation), Some(turn)) = (from.generation(), turn) {
            if let Some(session) = self.sessions.get_mut(&thread_id) {
                if session.generation == generation {
                    session.finished.insert(turn.clone());
                }
            }
        }
    }

    /// Opens the thread's run, or a turn of the provider's own, before `ev`
    /// is stored.
    pub fn enter(&mut self, thread_id: i64, from: From, ev: &HarnessEvent) {
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
                        turn: None,
                        settling: false,
                    },
                );
            }
            HarnessEvent::UnpromptedTurn { .. } => {
                self.failed_writes.remove(&thread_id);
                self.own.insert(thread_id, None);
            }
            HarnessEvent::TurnStarted { turn: Some(turn) } => {
                if let (From::Session(generation), Some(own @ None)) =
                    (from, self.own.get_mut(&thread_id))
                {
                    *own = Some((generation, turn.clone()));
                }
            }
            _ => {}
        }
    }

    /// The run `ev` belongs to: the thread's open one, unless the provider
    /// is in a turn of its own. A run's own terminal is always its.
    fn scope(&self, thread_id: i64, terminal: Option<Terminal>) -> Option<&Open> {
        match terminal {
            Some(Terminal::Run) => self.open.get(&thread_id),
            Some(_) => None,
            None if self.own.contains_key(&thread_id) => None,
            None => self.open.get(&thread_id),
        }
    }

    pub fn tags(&self, thread_id: i64, terminal: Option<Terminal>) -> Tags {
        self.scope(thread_id, terminal)
            .map(|o| Tags {
                run_id: Some(o.run_id),
                attempt_id: Some(o.attempt_id),
            })
            .unwrap_or_default()
    }

    /// The owner a row made from `ev` would get. The question's own row (or
    /// a continuation's) was written with its run, so it takes none here.
    pub fn owner(
        &self,
        thread_id: i64,
        ev: &HarnessEvent,
        terminal: Option<Terminal>,
    ) -> Option<store::Owner> {
        if matches!(
            ev,
            HarnessEvent::UserMessage { .. } | HarnessEvent::Continued { .. }
        ) {
            return None;
        }
        self.scope(thread_id, terminal).map(|o| store::Owner {
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

    /// The provider named its session: the open run's binding learns it.
    pub fn learn_session(
        &self,
        rt: &Runtime,
        pool: &SqlitePool,
        thread_id: i64,
        ev: &HarnessEvent,
        terminal: Option<Terminal>,
    ) {
        let (
            Some(open),
            HarnessEvent::SessionStarted {
                provider_session_id,
                ..
            },
        ) = (self.scope(thread_id, terminal), ev)
        else {
            return;
        };
        let bound = rt.block_on(store::bind_session(pool, open.run_id, provider_session_id));
        if let Err(e) = bound {
            elog!("[oculus] harness run {thread_id}: {e}");
        }
    }

    /// The run's terminal was seen and its settle write failed: until the
    /// write lands, no other terminal ends the run or frees its claim.
    pub fn settling(&mut self, thread_id: i64) {
        if let Some(open) = self.open.get_mut(&thread_id) {
            open.settling = true;
        }
    }

    /// A restart steer superseded the open run's attempt: `attempt_id`
    /// carries the run on, and its next send binds the turn afresh.
    pub fn restart(&mut self, thread_id: i64, attempt_id: i64) {
        if let Some(open) = self.open.get_mut(&thread_id) {
            open.attempt_id = attempt_id;
            open.turn = None;
            open.settling = false;
        }
    }

    /// The run settled.
    pub fn close(&mut self, thread_id: i64) {
        self.open.remove(&thread_id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn open_run(scope: &mut RunScope, thread: i64) {
        let run = store::RunStart {
            run_id: 7,
            attempt_id: 8,
            binding_id: 1,
            item_id: 1,
        };
        scope.enter(
            thread,
            From::App,
            &HarnessEvent::UserMessage {
                text: "q".into(),
                at: None,
                run: Some(run),
            },
        );
    }

    fn ended(turn: &str) -> HarnessEvent {
        HarnessEvent::TurnFinished {
            status: "completed".into(),
            turn: Some(turn.into()),
        }
    }

    #[test]
    fn a_bound_run_is_ended_only_by_its_own_turn_of_its_own_session() {
        let mut scope = RunScope::default();
        assert!(scope.admit(1, 4, &ended("a")));
        open_run(&mut scope, 1);
        scope.bind(1, 4, "b".into());
        let ends =
            |scope: &RunScope, from, turn: &str| scope.terminal(1, from, Some(&turn.to_string()));
        assert_eq!(ends(&scope, From::Session(4), "a"), Terminal::Stray);
        assert_eq!(ends(&scope, From::Session(3), "b"), Terminal::Stray);
        assert_eq!(ends(&scope, From::Session(4), "b"), Terminal::Run);
        // The app's own terminal names no turn: the send failed.
        assert_eq!(scope.terminal(1, From::App, None), Terminal::Run);
        // With nothing open the app's terminal only ends a turn; a session's
        // that names none ends nothing.
        scope.close(1);
        assert_eq!(scope.terminal(1, From::App, None), Terminal::Claim);
        assert_eq!(scope.terminal(1, From::Session(4), None), Terminal::Stray);
    }

    #[test]
    fn a_run_bound_to_no_turn_is_not_ended_by_a_turn_already_over() {
        let mut scope = RunScope::default();
        assert!(scope.admit(1, 4, &ended("a")));
        scope.finished(1, From::Session(4), Some(&"a".to_string()), Terminal::Stray);
        open_run(&mut scope, 1);
        let a = "a".to_string();
        let b = "b".to_string();
        assert_eq!(
            scope.terminal(1, From::Session(4), Some(&a)),
            Terminal::Stray,
            "a repeat of the last turn's end"
        );
        assert_eq!(
            scope.terminal(1, From::Session(4), Some(&b)),
            Terminal::Run,
            "the bridge never named the send's turn, so its session's next end is it"
        );
    }

    #[test]
    fn a_replaced_sessions_end_still_settles_the_run_bound_to_it() {
        let mut scope = RunScope::default();
        assert!(scope.admit(1, 4, &ended("a")));
        open_run(&mut scope, 1);
        scope.bind(1, 4, "b".into());
        assert!(scope.admit(1, 5, &ended("x")), "a newer session");
        assert!(!scope.admit(1, 4, &ended("a")));
        assert!(!scope.admit(1, 4, &HarnessEvent::UnpromptedTurn { after: Vec::new() }));
        assert!(scope.admit(1, 4, &ended("b")), "the bound run's own end");
    }
}
