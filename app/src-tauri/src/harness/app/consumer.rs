use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use sqlx::SqlitePool;
use tokio::runtime::Runtime;

use crate::harness::event::{GapCause, Source};
use crate::harness::{
    jobs, store, Generation, HarnessEvent, Provider, Queue, QueuedMessage, Release, SendOptions,
};

use super::drafts::Drafts;
use super::run_scope::{From, RunScope, Tags, Terminal};
use super::thought_clock::ThoughtClock;
use super::{BusMsg, Envelope};

/// External effects of the ordered consumer. Replay supplies a scratch pool
/// and checks every outbound operation; production retains its Tauri effects.
pub(in crate::harness) trait ConsumerIo {
    fn clock_time(&self) -> Option<std::time::Instant> {
        None
    }
    /// The clock work that waits is timed by (a draft's next write, a
    /// failed settle's next try).
    fn now(&self) -> Instant {
        Instant::now()
    }
    /// Ask for a `BusMsg::Wake` at `at`, sooner if another message comes
    /// first. Unanswered, waiting work is found by the next message.
    fn wake_at(&mut self, _at: Instant) {}
    fn open_pool(&mut self, rt: &Runtime) -> Option<SqlitePool>;
    fn name(
        &mut self,
        thread_id: i64,
        provider: Provider,
        chain: Vec<jobs::JobSelection>,
        seed: store::NamingSeed,
    );
    fn dispatch(&mut self, thread: i64, provider: Provider, msg: QueuedMessage, opts: SendOptions);
    fn emit(&mut self, envelope: Envelope);
}

pub(in crate::harness) fn consume(
    rx: impl IntoIterator<Item = BusMsg>,
    queue: Arc<Mutex<Queue>>,
    io: &mut impl ConsumerIo,
) {
    let mut consumer = Consumer {
        rt: tokio::runtime::Runtime::new().expect("tokio runtime"),
        pool: None,
        queue,
        clock: ThoughtClock::default(),
        runs: RunScope::default(),
        drafts: Drafts::default(),
        parked: HashMap::new(),
        sequence: first_sequence(),
    };
    for msg in rx {
        consumer.handle(msg, io);
        consumer.due(io);
    }
    let waiting = consumer.drafts.waiting();
    consumer.store_drafts(waiting);
}

/// The one ordered fold over every thread's events: what [`consume`] keeps
/// between messages.
struct Consumer {
    rt: Runtime,
    pool: Option<SqlitePool>,
    queue: Arc<Mutex<Queue>>,
    clock: ThoughtClock,
    runs: RunScope,
    drafts: Drafts,
    /// Terminals whose settle write has not landed yet, by thread.
    parked: HashMap<i64, Parked>,
    sequence: u64,
}

impl Consumer {
    fn handle(&mut self, msg: BusMsg, io: &mut impl ConsumerIo) {
        if self.pool.is_none() {
            self.pool = io.open_pool(&self.rt);
        }
        match msg {
            BusMsg::Event {
                thread_id,
                provider,
                generation,
                event,
            } => {
                let from = generation.map_or(From::App, From::Session);
                self.event(thread_id, provider, from, event, None, io);
            }
            // The question becomes an ordinary `UserMessage` once its run is
            // written (a continuation's, `Continued`); a failure writes
            // nothing and the caller does not send.
            BusMsg::Begin { intent, reply } => {
                let begun = match &self.pool {
                    Some(p) => self.rt.block_on(store::begin_run(p, &intent)),
                    None => Err("the database is not open".into()),
                };
                let _ = reply.send(begun.clone());
                let Ok(run) = begun else { return };
                // A card whose continuation state changed here; its snapshot follows the event.
                let mut card = None;
                let ev = match intent.continuation {
                    Some(proposal) => {
                        card = Some(proposal.clone());
                        HarnessEvent::Continued {
                            text: intent.text,
                            proposal,
                            run: Some(run),
                        }
                    }
                    None => HarnessEvent::UserMessage {
                        text: intent.text,
                        at: intent.at,
                        run: Some(run),
                    },
                };
                self.event(intent.thread_id, intent.provider, From::App, ev, card, io);
            }
            BusMsg::Unclaimed(thread_id, provider) => {
                let next = self.queue.lock().unwrap().next(thread_id);
                if let Some((msg, opts)) = next {
                    io.dispatch(thread_id, provider, msg, opts);
                }
            }
            BusMsg::Bound {
                thread_id,
                generation,
                turn,
            } => self.runs.bind(thread_id, generation, turn),
            BusMsg::Released {
                thread_id,
                provider,
                generation,
                reason,
            } => self.end_session(thread_id, provider, generation, reason, io),
            BusMsg::Wake => {}
        }
    }

    /// Work whose time has come, then the alarm for what still waits.
    fn due(&mut self, io: &mut impl ConsumerIo) {
        let now = io.now();
        let due = self.drafts.due(now);
        self.store_drafts(due);
        self.retry(now, io);
        let retries = self.parked.values().map(|parked| parked.due).min();
        if let Some(at) = self.drafts.next_due().into_iter().chain(retries).min() {
            io.wake_at(at);
        }
    }

    /// Write each run's draft, whole; one with no text left is removed.
    fn store_drafts(&mut self, drafts: Vec<(i64, String)>) {
        let Some(p) = &self.pool else { return };
        for (run_id, text) in drafts {
            let stored = match text.is_empty() {
                true => self.rt.block_on(store::clear_draft(p, run_id)),
                false => self.rt.block_on(store::set_draft(p, run_id, &text)),
            };
            if let Err(e) = stored {
                elog!("[oculus] harness draft {run_id}: {e}");
            }
        }
    }

    /// Answer text that has streamed and is not a row yet survives a
    /// restart (`store::recover`): gathered by message and written at most
    /// once per interval (`drafts.rs`). A committed message leaves it at
    /// once, so a restart cannot bring its text back a second time.
    fn draft(&mut self, run_id: i64, ev: &HarnessEvent, now: Instant) {
        let text = match ev {
            HarnessEvent::AssistantDelta { text, item } => {
                self.drafts.push(run_id, item.as_deref(), text, now)
            }
            HarnessEvent::AssistantMessage { source, .. } => {
                let item = source.as_ref().map(|s| s.id.as_str());
                self.drafts.commit(run_id, item, now)
            }
            _ => None,
        };
        if let Some(text) = text {
            self.store_drafts(vec![(run_id, text)]);
        }
    }

    /// A session is over. One rule for every bridge: a turn it left open is
    /// closed here, with the text that streamed kept as a partial reply. A
    /// stop or a replacement interrupts the turn and a dead process fails
    /// it; a deleted thread has nothing left to close, and a quit leaves the
    /// run to the next startup's repair.
    fn end_session(
        &mut self,
        thread_id: i64,
        provider: Provider,
        generation: Generation,
        reason: Release,
        io: &mut impl ConsumerIo,
    ) {
        let status = match reason {
            Release::Stopped | Release::Replaced => Some("interrupted"),
            Release::Exited => Some("failed"),
            Release::Deleted | Release::Quit => None,
        };
        if let Some(status) = status {
            let from = From::Ended(generation);
            let (run, own) = self.runs.left_open(thread_id, generation);
            if let Some((run_id, turn)) = run {
                for event in self.cut_short(run_id, status) {
                    self.event(thread_id, provider, from, event, None, io);
                }
                let finished = HarnessEvent::TurnFinished {
                    status: status.into(),
                    turn: Some(turn),
                };
                self.event(thread_id, provider, from, finished, None, io);
            }
            if let Some(turn) = own {
                let finished = HarnessEvent::TurnFinished {
                    status: status.into(),
                    turn,
                };
                self.event(thread_id, provider, from, finished, None, io);
            }
        }
        if reason == Release::Deleted {
            self.runs.forget(thread_id);
            self.parked.remove(&thread_id);
        }
        self.runs.end(thread_id, generation);
    }

    /// What is said for a run whose session ended under it: the text it had
    /// streamed, as a partial reply, and for a process that died, the gap
    /// and an error row.
    fn cut_short(&mut self, run_id: i64, status: &str) -> Vec<HarnessEvent> {
        let mut out = Vec::new();
        for (item, text) in self.drafts.take(run_id) {
            if text.trim().is_empty() {
                continue;
            }
            let id = item.unwrap_or_else(|| CUT_SHORT.into());
            out.push(HarnessEvent::AssistantMessage {
                text: text.trim().to_string(),
                source: Source::partial(id.clone()),
            });
            if status == "failed" {
                out.push(HarnessEvent::DeliveryGap {
                    source: Some(id),
                    cause: GapCause::MissingSnapshot,
                });
            }
        }
        if status == "failed" {
            out.push(HarnessEvent::error(SESSION_ENDED));
        }
        out
    }

    /// Store one event and tell the webview. `card` is a proposal whose
    /// continuation state the event changed; its snapshot follows.
    fn event(
        &mut self,
        thread_id: i64,
        provider: Provider,
        from: From,
        mut ev: HarnessEvent,
        card: Option<String>,
        io: &mut impl ConsumerIo,
    ) {
        // A session that has ended or been replaced is heard no more: its
        // late events belong to no run the thread has now.
        if let (true, From::Session(generation)) = (thread_id > 0, from) {
            if !self.runs.admit(thread_id, generation, &ev) {
                elog!(
                    "[oculus] harness thread {thread_id}: dropped {} from ended session {generation}",
                    kind(&ev)
                );
                return;
            }
            // The process is gone, and so is its session.
            if matches!(ev, HarnessEvent::Exited { .. }) {
                self.end_session(thread_id, provider, generation, Release::Exited, io);
            }
        }
        // Whose turn a terminal ends. One for a turn this thread does not
        // have open (another turn's, or a repeated `result`) settles nothing
        // and must not release a queued message.
        let terminal =
            match &ev {
                HarnessEvent::TurnFinished { turn, .. } if thread_id > 0 => Some(
                    self.runs
                        .terminal(thread_id, from, turn.as_ref(), &self.queue),
                ),
                _ => None,
            };
        if terminal == Some(Terminal::Stray) {
            if let (From::Session(generation), HarnessEvent::TurnFinished { turn, .. }) =
                (from, &ev)
            {
                elog!(
                    "[oculus] harness thread {thread_id}: ignored the end of turn {turn:?} (session {generation}), which is not open here"
                );
            }
            return;
        }
        // After the stray check: a turn end that ends nothing here must not
        // stop the clock on the open turn's thought.
        match io.clock_time() {
            Some(now) => self.clock.observe_at(thread_id, &mut ev, now),
            None => self.clock.observe(thread_id, &mut ev),
        }
        let mut item_id = None;
        let mut entity_id = None;
        let mut settled = Vec::new();
        self.runs.enter(thread_id, from, &ev);
        let tags = self.runs.tags(thread_id, terminal);
        let (pool, now) = (self.pool.clone(), io.now());
        if let Some(p) = &pool {
            if thread_id > 0 {
                let owner = self.runs.owner(thread_id, &ev, terminal);
                match self
                    .rt
                    .block_on(store::apply_event(p, thread_id, &ev, owner.as_ref()))
                {
                    Ok(stored) => {
                        item_id = stored.item_id;
                        if stored.inserted && owner.is_some() {
                            self.runs.written(thread_id);
                        }
                        entity_id = stored.entity;
                    }
                    // A row that could not be saved is not shown as if it
                    // were: the run gets one error row and a gap instead.
                    Err(e) if makes_row(&ev) => {
                        elog!("[oculus] harness store: {e}");
                        if !self.runs.first_storage_failure(thread_id) {
                            return;
                        }
                        ev = HarnessEvent::error(STORAGE_FAILED);
                        let gap = HarnessEvent::DeliveryGap {
                            source: None,
                            cause: GapCause::StorageFailure,
                        };
                        let owner = self.runs.owner(thread_id, &ev, terminal);
                        for event in [&gap, &ev] {
                            match self.rt.block_on(store::apply(
                                p,
                                thread_id,
                                event,
                                owner.as_ref(),
                            )) {
                                Ok(id) => item_id = id,
                                Err(e) => elog!("[oculus] harness store: {e}"),
                            }
                        }
                        if item_id.is_some() {
                            self.runs.written(thread_id);
                        }
                    }
                    Err(e) => elog!("[oculus] harness store: {e}"),
                }
                settled = match (terminal, tags.run_id, tags.attempt_id, &ev) {
                    (
                        Some(Terminal::Run),
                        Some(run_id),
                        Some(attempt_id),
                        HarnessEvent::TurnFinished { status, .. },
                    ) => match self.settle(p, thread_id, run_id, attempt_id, status) {
                        Ok(settled) => settled,
                        // The one write that may not be lost: nobody hears
                        // of the terminal until it has landed (`retry`).
                        Err(e) => {
                            elog!("[oculus] harness run {run_id}: settle failed, retrying: {e}");
                            self.runs.settling(thread_id);
                            let parked = Parked {
                                run_id,
                                attempt_id,
                                status: status.clone(),
                                wait: RETRY_FIRST,
                                due: now + RETRY_FIRST,
                                stored: Stored {
                                    thread_id,
                                    provider,
                                    from,
                                    ev,
                                    card,
                                    tags,
                                    item_id,
                                    entity_id,
                                    terminal,
                                },
                            };
                            self.parked.insert(thread_id, parked);
                            return;
                        }
                    },
                    (_, run_id, ..) => {
                        self.runs
                            .learn_session(&self.rt, p, thread_id, &ev, terminal);
                        if let Some(run_id) = run_id {
                            self.draft(run_id, &ev, now);
                        }
                        Vec::new()
                    }
                };
            }
        }
        let stored = Stored {
            thread_id,
            provider,
            from,
            ev,
            card,
            tags,
            item_id,
            entity_id,
            terminal,
        };
        self.publish(stored, settled, io);
    }

    /// Close the run on its own terminal (`store::settle_run`) and say what
    /// was decided. An `Err` wrote nothing and left the run open.
    fn settle(
        &mut self,
        pool: &SqlitePool,
        thread_id: i64,
        run_id: i64,
        attempt_id: i64,
        status: &str,
    ) -> Result<Vec<(HarnessEvent, Option<i64>)>, String> {
        let settlement = self
            .rt
            .block_on(store::settle_run(pool, run_id, attempt_id, status))?;
        self.runs.close(thread_id);
        // Settlement removed the stored draft with the run.
        self.drafts.take(run_id);
        let mut out = Vec::new();
        if let Some(s) = settlement {
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
        Ok(out)
    }

    /// Settle writes that failed and are due another try. Each waits twice
    /// as long as the last, up to [`RETRY_MAX`], until it lands; its
    /// terminal is published then, and until then the thread stays claimed,
    /// so the chat's next message does not start.
    fn retry(&mut self, now: Instant, io: &mut impl ConsumerIo) {
        let Some(pool) = self.pool.clone() else {
            return;
        };
        let due: Vec<i64> = self
            .parked
            .iter()
            .filter(|(_, parked)| parked.due <= now)
            .map(|(thread_id, _)| *thread_id)
            .collect();
        for thread_id in due {
            let Some(mut parked) = self.parked.remove(&thread_id) else {
                continue;
            };
            let (run_id, attempt_id) = (parked.run_id, parked.attempt_id);
            match self.settle(&pool, thread_id, run_id, attempt_id, &parked.status) {
                Ok(settled) => self.publish(parked.stored, settled, io),
                Err(e) => {
                    parked.wait = (parked.wait * 2).min(RETRY_MAX);
                    parked.due = now + parked.wait;
                    elog!(
                        "[oculus] harness run {run_id}: settle failed again, next try in {:?}: {e}",
                        parked.wait
                    );
                    self.parked.insert(thread_id, parked);
                }
            }
        }
    }

    /// Tell the webview about a stored event, and do what follows from it:
    /// the settlement's events first, naming, the queue, a card's
    /// continuation.
    fn publish(
        &mut self,
        stored: Stored,
        settled: Vec<(HarnessEvent, Option<i64>)>,
        io: &mut impl ConsumerIo,
    ) {
        let Stored {
            thread_id,
            provider,
            from,
            ev,
            mut card,
            tags,
            item_id,
            entity_id,
            terminal,
        } = stored;
        for (event, id) in settled {
            io.emit(Envelope {
                thread_id,
                provider,
                item_id: id,
                run_id: tags.run_id,
                attempt_id: tags.attempt_id,
                entity_id: None,
                sequence: next_sequence(&mut self.sequence),
                event,
            });
        }
        if let Some(p) = &self.pool {
            if let Err(e) = self.rt.block_on(store::save_rate_limits(p, provider, &ev)) {
                elog!("[oculus] harness rate limits: {e}");
            }
            // A finished first exchange is when a thread is named. The
            // claim is atomic (asks at most once); the naming turn runs
            // off this loop, which every thread's events wait behind.
            if thread_id > 0
                && matches!(&ev, HarnessEvent::TurnFinished { status, .. } if status == "completed")
            {
                match self.rt.block_on(store::claim_naming(p, thread_id)) {
                    Ok(Some(seed)) => {
                        let chain = self.rt.block_on(jobs::chain(p, jobs::Job::ThreadNaming));
                        // An empty list switches naming off; the thread stays as it is.
                        if !chain.is_empty() {
                            io.name(thread_id, provider, chain, seed);
                        }
                    }
                    Ok(None) => {}
                    Err(e) => elog!("[oculus] harness title: {e}"),
                }
            }
        }
        // A turn the provider started itself holds the thread like a
        // send, so a message typed meanwhile waits its own turn.
        if thread_id > 0 && matches!(&ev, HarnessEvent::UnpromptedTurn { .. }) {
            self.queue.lock().unwrap().hold(thread_id);
        }
        // A closed turn releases the next queued message; its send is
        // spawned, since a send may start a process.
        if let (Some(was), HarnessEvent::TurnFinished { turn, .. }) = (terminal, &ev) {
            self.runs.finished(thread_id, from, turn.as_ref(), was);
            let next = self.queue.lock().unwrap().next(thread_id);
            if let Some((msg, opts)) = next {
                io.dispatch(thread_id, provider, msg, opts);
            }
        }

        // A settled card with a follow-up claims its one continuation: it
        // goes out now on an idle thread, or waits its turn like a message.
        if let (HarnessEvent::CardSettled { proposal }, Some(p)) = (&ev, &self.pool) {
            match self
                .rt
                .block_on(super::continuation::claim(p, thread_id, proposal))
            {
                Ok(Some(line)) => {
                    card = Some(proposal.clone());
                    let opts = SendOptions {
                        continuation: Some(proposal.clone()),
                        ..SendOptions::default()
                    };
                    let mut q = self.queue.lock().unwrap();
                    if q.try_claim(thread_id) {
                        drop(q);
                        io.dispatch(thread_id, provider, QueuedMessage::new(&line), opts);
                    } else {
                        q.push(thread_id, &line, &opts);
                    }
                }
                Ok(None) => {}
                Err(e) => elog!("[oculus] card continuation {proposal}: {e}"),
            }
        }

        io.emit(Envelope {
            thread_id,
            provider,
            item_id,
            run_id: tags.run_id,
            attempt_id: tags.attempt_id,
            entity_id,
            sequence: next_sequence(&mut self.sequence),
            event: ev,
        });
        if let (Some(id), Some(p)) = (card, &self.pool) {
            match self.rt.block_on(crate::db::proposals::get(p, &id)) {
                Ok(proposal) => io.emit(Envelope {
                    thread_id,
                    provider,
                    item_id: None,
                    run_id: None,
                    attempt_id: None,
                    entity_id: None,
                    sequence: next_sequence(&mut self.sequence),
                    event: HarnessEvent::Proposal { proposal },
                }),
                Err(e) => elog!("[oculus] card continuation {id}: {e}"),
            }
        }
    }
}

/// An event that has been stored and not yet told to the webview.
struct Stored {
    thread_id: i64,
    provider: Provider,
    from: From,
    ev: HarnessEvent,
    /// A proposal whose continuation state the event changed.
    card: Option<String>,
    tags: Tags,
    item_id: Option<i64>,
    entity_id: Option<String>,
    terminal: Option<Terminal>,
}

/// A run's terminal whose settle write failed, waiting for its next try.
struct Parked {
    run_id: i64,
    attempt_id: i64,
    status: String,
    wait: Duration,
    due: Instant,
    stored: Stored,
}

/// The first wait before a failed settle is tried again, and the longest.
const RETRY_FIRST: Duration = Duration::from_millis(100);
const RETRY_MAX: Duration = Duration::from_secs(30);

/// An event's wire name, for a log line.
fn kind(ev: &HarnessEvent) -> String {
    serde_json::to_value(ev)
        .ok()
        .and_then(|v| v["type"].as_str().map(String::from))
        .unwrap_or_default()
}

/// The error row of a turn whose process ended without closing it.
const SESSION_ENDED: &str = "The agent's process ended before it finished this turn.";

/// The source id of the partial reply made from a cut-short run's draft,
/// when the provider named no message for it.
const CUT_SHORT: &str = "cut-short";

const STORAGE_FAILED: &str =
    "Oculus could not save part of this reply, so some of it may be missing.";

/// Events that become a row, so a failed write would leave the webview
/// showing what a reload cannot. `UnpromptedTurn` still holds its thread.
fn makes_row(ev: &HarnessEvent) -> bool {
    matches!(
        ev,
        HarnessEvent::AssistantMessage { .. }
            | HarnessEvent::Thinking { .. }
            | HarnessEvent::ToolStarted { .. }
            | HarnessEvent::PermissionNeeded { .. }
            | HarnessEvent::Compaction { .. }
    )
}

/// Envelope order across the app's life: microseconds at start, then +1, so a
/// restarted app still counts up and the webview can drop a repeat.
fn first_sequence() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_micros() as u64)
        .unwrap_or_default()
}

fn next_sequence(sequence: &mut u64) -> u64 {
    *sequence += 1;
    *sequence
}
