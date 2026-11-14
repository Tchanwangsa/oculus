//! The student's commands on the consumer: receipt, decide, commit, then
//! tell the webview and leave the provider call to an effect. One command
//! is taken from start to commit before anything else on the bus is read,
//! so the consumer is the chat's lock.

use crate::harness::core::commit::{self, Applied, Began, Change, Receipt};
use crate::harness::core::decide::{decide, Answer, Ask, Decision, Plan};
use crate::harness::core::driver::{self, Calls, Step};
use crate::harness::core::effect::Effect;
use crate::harness::core::state::{self, Live};
use crate::harness::core::{receipts, Command, Kind, Outcome};
use crate::harness::store::queue::Waiting;
use crate::harness::{store, HarnessEvent, Provider, Release, SendOptions};

use super::super::run_scope::From;
use super::super::send::Sent;
use super::super::Reply;
use super::{Consumer, ConsumerIo};

/// A command that asked its provider something and commits on the answer.
pub(super) struct Asking {
    command: Command,
    provider: Provider,
    reply: Reply,
}

impl Consumer {
    /// Take one command through to its commit, or to the question it has
    /// to ask its provider first.
    pub(super) fn command(&mut self, command: Command, reply: Reply, io: &mut impl ConsumerIo) {
        match self.accept(&command, io) {
            Ok(Accepted::Done(outcome)) => {
                let _ = reply.send(Ok(outcome));
            }
            Ok(Accepted::Asked(provider)) => {
                if let Some(thread_id) = command.thread_id {
                    let asking = Asking {
                        command,
                        provider,
                        reply,
                    };
                    self.asking.insert(thread_id, asking);
                }
            }
            Err(e) => {
                let _ = reply.send(Err(e));
            }
        }
    }

    fn accept(&mut self, command: &Command, io: &mut impl ConsumerIo) -> Result<Accepted, String> {
        let pool = self.pool.clone().ok_or("the database is not open")?;
        // Before anything else, a thread's creation included: an id already
        // accepted returns what it did the first time.
        if let Some(receipt) = self.rt.block_on(receipts::find(&pool, &command.id))? {
            if command.thread_id.is_some_and(|t| t != receipt.thread_id) {
                return Err(format!(
                    "command {} was already used on another thread",
                    command.id
                ));
            }
            return Ok(Accepted::Done(receipt.outcome));
        }
        if self.asking.values().any(|a| a.command.id == command.id) {
            return Err(format!("command {} is still running", command.id));
        }
        let live = self.live(command.thread_id);
        let state = self.rt.block_on(state::read(&pool, command, live))?;
        let provider = match (&state.thread, &command.kind) {
            (Some(row), _) => row.provider,
            (None, Kind::Send { provider, .. }) => *provider,
            (None, _) => return Err("a command names a thread".into()),
        };
        match decide(&state, command, None)? {
            Decision::Commit(plan) => {
                let outcome = self.commit(Some(command), command.thread_id, provider, plan, io)?;
                // A turn the CLI started itself has no run for the driver to
                // read a stop from: it is asked directly.
                let own_turn = self.runs.own_open(outcome.thread_id);
                if let (Kind::Stop { run_id: None }, true) = (&command.kind, own_turn) {
                    io.perform(Effect::Interrupt {
                        thread_id: outcome.thread_id,
                    });
                }
                // Whatever the saved runs now imply: the new run's start, or
                // a message that joined an idle chat's queue going out.
                self.drive(outcome.thread_id, provider, io);
                Ok(Accepted::Done(outcome))
            }
            Decision::Ask(Ask::Rewind(ask)) => {
                // The chat is held while its provider is asked (`held`): a
                // message sent meanwhile waits, and cannot be rewound away
                // unseen.
                io.perform(Effect::Rewind {
                    command: command.id.clone(),
                    ask,
                });
                Ok(Accepted::Asked(provider))
            }
        }
    }

    /// The provider answered what a command asked: decide it again with the
    /// answer, commit, and let the chat go.
    pub(super) fn answer(
        &mut self,
        thread_id: i64,
        command: &str,
        answer: Answer,
        io: &mut impl ConsumerIo,
    ) {
        if self.asking.get(&thread_id).map(|a| a.command.id.as_str()) != Some(command) {
            elog!("[oculus] harness thread {thread_id}: an answer for {command}, which is not waiting");
            return;
        }
        let Some(asking) = self.asking.remove(&thread_id) else {
            return;
        };
        let decided = self.resume(&asking, answer, io);
        let _ = asking.reply.send(decided);
        // The chat is no longer held for the ask: an edit's run starts, or
        // whatever waited meanwhile does.
        self.drive(thread_id, asking.provider, io);
    }

    fn resume(
        &mut self,
        asking: &Asking,
        answer: Answer,
        io: &mut impl ConsumerIo,
    ) -> Result<Outcome, String> {
        let pool = self.pool.clone().ok_or("the database is not open")?;
        let command = &asking.command;
        let live = self.live(command.thread_id);
        let state = self.rt.block_on(state::read(&pool, command, live))?;
        match decide(&state, command, Some(answer))? {
            Decision::Commit(plan) => {
                self.commit(Some(command), command.thread_id, asking.provider, plan, io)
            }
            Decision::Ask(_) => Err("the provider was already asked".into()),
        }
    }

    /// What only this process knows about a chat, for its deciders.
    fn live(&self, thread_id: Option<i64>) -> Live {
        Live {
            held: thread_id.is_some_and(|id| self.held(id)),
            own_turn: thread_id.is_some_and(|id| self.runs.own_open(id)),
        }
    }

    /// Something no row shows holds the chat: a turn its CLI started
    /// itself, a rewind waiting on the provider, or a settle that has not
    /// landed.
    fn held(&self, thread_id: i64) -> bool {
        self.runs.own_open(thread_id)
            || self.asking.contains_key(&thread_id)
            || self.parked.contains_key(&thread_id)
    }

    /// Save a plan for the chat `thread_id` (`None` when the plan makes
    /// it), then announce what it changed and hand its provider calls on.
    /// An `Err` saved nothing. `command` is `None` for what the app does
    /// itself, which takes no receipt.
    fn commit(
        &mut self,
        command: Option<&Command>,
        thread_id: Option<i64>,
        provider: Provider,
        plan: Plan,
        io: &mut impl ConsumerIo,
    ) -> Result<Outcome, String> {
        let pool = self.pool.clone().ok_or("the database is not open")?;
        let sends = plan
            .changes
            .iter()
            .any(|c| matches!(c, Change::Begin { .. }));
        let receipt = command.map(|c| Receipt {
            id: &c.id,
            command: c.kind.name(),
        });
        let committed = self
            .rt
            .block_on(commit::run(&pool, receipt, thread_id, plan));
        let committed = match committed {
            Ok(committed) => committed,
            Err(error) => {
                // A student's send that could not be saved says so in the
                // chat, and closes as a failed turn: an error row, then its
                // terminal. What the app starts itself is its caller's to
                // report (`promote`).
                if let (Some(thread_id), true) = (thread_id, sends && command.is_some()) {
                    self.refuse(thread_id, provider, &error, io);
                }
                return Err(error);
            }
        };
        let thread_id = committed.outcome.thread_id;
        if committed.revision > 0 {
            self.revisions.insert(thread_id, committed.revision);
        }
        for applied in committed.applied {
            self.applied(thread_id, provider, applied, io);
        }
        Ok(committed.outcome)
    }

    fn applied(
        &mut self,
        thread_id: i64,
        provider: Provider,
        applied: Applied,
        io: &mut impl ConsumerIo,
    ) {
        match applied {
            Applied::Began(began) => {
                let Began {
                    provider,
                    run,
                    text,
                    at,
                    continuation,
                    waited,
                } = *began;
                // It left the queue before its question appears.
                if waited {
                    self.tell_queue(thread_id, provider, io);
                }
                // The question becomes an ordinary `UserMessage` now that its
                // run is written (a continuation's, `Continued`, and its
                // card's snapshot follows). The driver sends it.
                let event = match &continuation {
                    Some(proposal) => HarnessEvent::Continued {
                        text,
                        proposal: proposal.clone(),
                        run: Some(run),
                    },
                    None => HarnessEvent::UserMessage {
                        text,
                        at,
                        run: Some(run),
                    },
                };
                self.event(thread_id, provider, From::App, event, continuation, io);
            }
            Applied::Queue => self.tell_queue(thread_id, provider, io),
            Applied::Cards(cards) => {
                for card in cards {
                    self.snapshot(thread_id, provider, &card, io);
                }
            }
            Applied::Stopping(run_id) => {
                let stopping = HarnessEvent::StopRequested { run_id };
                self.event(thread_id, provider, From::App, stopping, None, io);
            }
            Applied::Truncated { item_id, context } => {
                let rewound = HarnessEvent::Rewound {
                    from_item_id: item_id,
                    context,
                };
                self.event(thread_id, provider, From::App, rewound, None, io);
            }
            Applied::Deleted => {
                // Whatever its session still says is nobody's: the rows are
                // gone, and a late event must not write new ones.
                self.runs.forget(thread_id);
                self.runs.silence(thread_id);
                self.parked.remove(&thread_id);
                self.revisions.remove(&thread_id);
                self.own_turns.lock().unwrap().forget(thread_id);
                io.perform(Effect::Release {
                    thread_id,
                    reason: Release::Deleted,
                });
            }
        }
    }

    /// A send that wrote nothing: the chat gets an error row and a failed
    /// turn's end, which marks the thread failed.
    fn refuse(
        &mut self,
        thread_id: i64,
        provider: Provider,
        error: &str,
        io: &mut impl ConsumerIo,
    ) {
        self.event(
            thread_id,
            provider,
            From::App,
            HarnessEvent::error(error),
            None,
            io,
        );
        let finished = HarnessEvent::TurnFinished {
            status: "failed".into(),
            turn: None,
        };
        self.event(thread_id, provider, From::App, finished, None, io);
    }

    /// The student's waiting messages, told whole: the webview's list is a
    /// mirror of the queued runs.
    fn tell_queue(&mut self, thread_id: i64, provider: Provider, io: &mut impl ConsumerIo) {
        let Some(pool) = self.pool.clone() else {
            return;
        };
        match self.rt.block_on(store::queue::list(&pool, thread_id)) {
            Ok(messages) => {
                let queue = HarnessEvent::Queue { messages };
                self.event(thread_id, provider, From::App, queue, None, io);
            }
            Err(e) => elog!("[oculus] harness thread {thread_id}: queue: {e}"),
        }
    }

    /// Write and send a run the app starts itself: a message that waited
    /// (`queued`, its run), or a card's continuation. No receipt of the
    /// command kind: nobody retries it, and the run's own receipt is fresh.
    pub(super) fn begin(
        &mut self,
        thread_id: i64,
        provider: Provider,
        (text, options): (String, SendOptions),
        queued: Option<i64>,
        io: &mut impl ConsumerIo,
    ) -> Result<(), String> {
        let plan = Plan {
            changes: vec![Change::Begin {
                provider,
                text,
                options,
                receipt: store::fresh_receipt(),
                queued,
            }],
            text: None,
        };
        self.commit(None, Some(thread_id), provider, plan, io)
            .map(|_| ())
    }

    /// A card's continuation, or anything else the app queues itself,
    /// joins the back of the thread's queue.
    pub(super) fn enqueue(
        &mut self,
        thread_id: i64,
        provider: Provider,
        (text, options): (String, SendOptions),
        io: &mut impl ConsumerIo,
    ) -> Result<(), String> {
        let plan = Plan {
            changes: vec![Change::Enqueue {
                provider,
                text,
                options,
            }],
            text: None,
        };
        self.commit(None, Some(thread_id), provider, plan, io)
            .map(|_| ())
    }

    /// Take the steps the chat's saved runs imply (`core::driver`): hand a
    /// `starting` run to its start step, ask the provider to stop a run the
    /// student stopped, or, with nothing open, start the first waiting run
    /// that is not held. Called after every commit, every
    /// turn's end and every effect's report; it reads the rows each time, so
    /// calling it when nothing changed does nothing.
    pub(super) fn drive(&mut self, thread_id: i64, provider: Provider, io: &mut impl ConsumerIo) {
        loop {
            let Some(pool) = self.pool.clone() else {
                return;
            };
            let saved = match self.rt.block_on(driver::read(&pool, thread_id)) {
                Ok(saved) => saved,
                Err(e) => {
                    elog!("[oculus] harness thread {thread_id}: driver: {e}");
                    return;
                }
            };
            let open = saved.open.as_ref().map(|open| open.run.run_id);
            let calls = Calls {
                started: open.is_some_and(|run| self.started.contains(&run)),
                accepted: open.is_some_and(|run| self.accepted.contains(&run)),
                asked: open.is_some_and(|run| self.asked.contains(&run)),
                held: self.held(thread_id),
            };
            match driver::step(thread_id, provider, saved, calls) {
                Step::Start(start) => {
                    self.started.insert(start.run.run_id);
                    io.perform(Effect::Start(start));
                    return;
                }
                Step::Interrupt(run_id) => {
                    self.asked.insert(run_id);
                    io.perform(Effect::Interrupt { thread_id });
                    return;
                }
                Step::Promote(waiting) => {
                    if !self.promote(thread_id, provider, *waiting, io) {
                        return;
                    }
                }
                Step::Wait => return,
            }
        }
    }

    /// A waiting run's turn has come: it becomes `starting`. A continuation
    /// whose card lost its claim (a rewind took the card) starts nothing and
    /// shows nothing, and a run that cannot be written leaves the queue as
    /// failed with an error row; either way the driver looks again. False
    /// when the queue could not be changed at all, which ends the round.
    fn promote(
        &mut self,
        thread_id: i64,
        provider: Provider,
        waiting: Waiting,
        io: &mut impl ConsumerIo,
    ) -> bool {
        let run_id = waiting.run_id;
        let send = (waiting.text, waiting.options);
        let Err(refused) = self.begin(thread_id, provider, send, Some(run_id), io) else {
            return true;
        };
        let lost = refused == store::NOT_CLAIMED;
        if !lost {
            let error = format!("Oculus could not start a waiting message: {}", refused);
            elog!("[oculus] harness thread {thread_id}: {error}");
            self.event(
                thread_id,
                provider,
                From::App,
                HarnessEvent::error(error),
                None,
                io,
            );
        }
        let close = Plan {
            changes: vec![Change::CloseQueued {
                run_id,
                failed: !lost,
            }],
            text: None,
        };
        // A database that refuses this too is not asked again in a loop.
        self.commit(None, Some(thread_id), provider, close, io)
            .is_ok()
    }

    /// A run's start step reported. Accepted: the provider's turn will say
    /// the rest. Withdrawn: the run was stopped before the provider heard
    /// of it, so it ends here, interrupted. Failed: the failure is the
    /// run's, an error row and a failed end. Each closes only the run it
    /// names, and only while that run is still the chat's open one.
    pub(super) fn sent(
        &mut self,
        thread_id: i64,
        provider: Provider,
        run_id: i64,
        outcome: Sent,
        io: &mut impl ConsumerIo,
    ) {
        let open = self.runs.open_run(thread_id) == Some(run_id);
        let status = match outcome {
            Sent::Accepted => {
                if open {
                    self.accepted.insert(run_id);
                }
                None
            }
            _ if !open => {
                elog!("[oculus] harness run {run_id}: its start ended ({outcome:?}) after the run did");
                None
            }
            Sent::Withdrawn => Some("interrupted"),
            Sent::Failed(error) => {
                self.event(
                    thread_id,
                    provider,
                    From::App,
                    HarnessEvent::error(error),
                    None,
                    io,
                );
                Some("failed")
            }
        };
        match status {
            Some(status) => {
                let finished = HarnessEvent::TurnFinished {
                    status: status.into(),
                    turn: None,
                };
                self.event(thread_id, provider, From::App, finished, None, io);
            }
            None => self.drive(thread_id, provider, io),
        }
    }
}

enum Accepted {
    Done(Outcome),
    /// The provider was asked; the command commits on its answer.
    Asked(Provider),
}
