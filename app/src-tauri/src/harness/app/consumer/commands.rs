//! The student's commands on the consumer: receipt, decide, commit, then
//! tell the webview and leave the provider call to an effect. One command
//! is taken from start to commit before anything else on the bus is read,
//! so the consumer is the chat's lock.

use crate::harness::core::commit::{self, Applied, Began, Change, Receipt};
use crate::harness::core::decide::{decide, Answer, Ask, Decision, Plan};
use crate::harness::core::effect::{Effect, Start};
use crate::harness::core::state::{self, Live};
use crate::harness::core::{receipts, Command, Kind, Outcome};
use crate::harness::{store, HarnessEvent, Provider, Release, SendOptions};

use super::super::run_scope::From;
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
                let outcome = self
                    .commit(Some(command), command.thread_id, provider, plan, io)
                    .map_err(|failed| failed.error)?;
                if matches!(command.kind, Kind::Stop) {
                    io.perform(Effect::Interrupt {
                        thread_id: outcome.thread_id,
                    });
                }
                // A message that joined an idle chat's queue goes out now.
                self.promote(outcome.thread_id, provider, io);
                Ok(Accepted::Done(outcome))
            }
            Decision::Ask(Ask::Rewind(ask)) => {
                // The chat is held while its provider is asked: a message
                // sent meanwhile waits, and cannot be rewound away unseen.
                self.queue.lock().unwrap().try_claim(ask.thread_id);
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
        // The hold taken to ask passes to whatever waited meanwhile, unless
        // an edit's run now has it, or a failed send's closing turn already
        // let it go.
        let held = match &decided {
            Ok(outcome) => outcome.run.is_none(),
            Err(failed) => !failed.closed,
        };
        let _ = asking.reply.send(decided.map_err(|failed| failed.error));
        if held {
            self.start_next(thread_id, asking.provider, io);
        }
    }

    fn resume(
        &mut self,
        asking: &Asking,
        answer: Answer,
        io: &mut impl ConsumerIo,
    ) -> Result<Outcome, Failed> {
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
            busy: thread_id.is_some_and(|id| self.queue.lock().unwrap().is_busy(id)),
        }
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
    ) -> Result<Outcome, Failed> {
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
                let closed = match thread_id {
                    Some(thread_id) if sends && command.is_some() => {
                        self.refuse(thread_id, provider, &error, io);
                        true
                    }
                    _ => false,
                };
                return Err(Failed { error, closed });
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
                    options,
                    waited,
                    respawn,
                } = *began;
                self.queue.lock().unwrap().try_claim(thread_id);
                // It left the queue before its question appears.
                if waited {
                    self.tell_queue(thread_id, provider, io);
                }
                // The question becomes an ordinary `UserMessage` now that its
                // run is written (a continuation's, `Continued`, and its
                // card's snapshot follows).
                let (event, card) = match &options.continuation {
                    Some(proposal) => (
                        HarnessEvent::Continued {
                            text: text.clone(),
                            proposal: proposal.clone(),
                            run: Some(run),
                        },
                        Some(proposal.clone()),
                    ),
                    None => (
                        HarnessEvent::UserMessage {
                            text: text.clone(),
                            at: options.at,
                            run: Some(run),
                        },
                        None,
                    ),
                };
                self.event(thread_id, provider, From::App, event, card, io);
                io.perform(Effect::Start(Box::new(Start {
                    thread_id,
                    provider,
                    run,
                    text,
                    options,
                    respawn,
                })));
            }
            Applied::Queue => self.tell_queue(thread_id, provider, io),
            Applied::Cards(cards) => {
                for card in cards {
                    self.snapshot(thread_id, provider, &card, io);
                }
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
                self.queue.lock().unwrap().forget(thread_id);
                io.perform(Effect::Release {
                    thread_id,
                    reason: Release::Deleted,
                });
            }
        }
    }

    /// A send that wrote nothing: the chat gets an error row and a failed
    /// turn's end, which also lets go whatever the send had claimed.
    fn refuse(
        &mut self,
        thread_id: i64,
        provider: Provider,
        error: &str,
        io: &mut impl ConsumerIo,
    ) {
        self.queue.lock().unwrap().try_claim(thread_id);
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
    ) -> Result<(), Failed> {
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
    ) -> Result<(), Failed> {
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

    /// A turn of the thread's closed: once nothing else holds it, the next
    /// waiting run goes out.
    pub(super) fn start_next(
        &mut self,
        thread_id: i64,
        provider: Provider,
        io: &mut impl ConsumerIo,
    ) {
        if self.queue.lock().unwrap().release(thread_id) {
            self.promote(thread_id, provider, io);
        }
    }

    /// Start the first waiting run that is not held, if the thread is free.
    /// Read from the saved queue each time, so nothing a notification
    /// missed is left behind. A continuation whose card lost its claim (a
    /// rewind took the card) starts nothing and shows nothing, and a run
    /// that cannot be written leaves the queue as failed with an error row;
    /// the thread then passes to what waits behind it.
    pub(super) fn promote(&mut self, thread_id: i64, provider: Provider, io: &mut impl ConsumerIo) {
        loop {
            if self.queue.lock().unwrap().is_busy(thread_id) {
                return;
            }
            let Some(pool) = self.pool.clone() else {
                return;
            };
            let waiting = match self.rt.block_on(store::queue::next(&pool, thread_id)) {
                Ok(Some(waiting)) => waiting,
                Ok(None) => return,
                Err(e) => {
                    elog!("[oculus] harness thread {thread_id}: queue: {e}");
                    return;
                }
            };
            let run_id = waiting.run_id;
            let send = (waiting.text, waiting.options);
            let Err(refused) = self.begin(thread_id, provider, send, Some(run_id), io) else {
                return;
            };
            let lost = refused.error == store::NOT_CLAIMED;
            if !lost {
                let error = format!(
                    "Oculus could not start a waiting message: {}",
                    refused.error
                );
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
            if self
                .commit(None, Some(thread_id), provider, close, io)
                .is_err()
            {
                return;
            }
        }
    }
}

/// A commit that saved nothing.
pub(super) struct Failed {
    pub error: String,
    /// The failure was told as a failed turn's end, which let go of
    /// whatever held the chat.
    pub closed: bool,
}

impl std::convert::From<String> for Failed {
    fn from(error: String) -> Self {
        Failed {
            error,
            closed: false,
        }
    }
}

impl std::convert::From<&str> for Failed {
    fn from(error: &str) -> Self {
        Failed::from(error.to_string())
    }
}

enum Accepted {
    Done(Outcome),
    /// The provider was asked; the command commits on its answer.
    Asked(Provider),
}
