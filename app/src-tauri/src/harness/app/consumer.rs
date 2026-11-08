use std::sync::{Arc, Mutex};

use sqlx::SqlitePool;
use tokio::runtime::Runtime;

use crate::harness::event::GapCause;
use crate::harness::{jobs, store, HarnessEvent, Provider, Queue, QueuedMessage, SendOptions};

use super::run_scope::RunScope;
use super::thought_clock::ThoughtClock;
use super::{BusMsg, Envelope};

/// External effects of the ordered consumer. Replay supplies a scratch pool
/// and checks every outbound operation; production retains its Tauri effects.
pub(in crate::harness) trait ConsumerIo {
    fn clock_time(&self) -> Option<std::time::Instant> {
        None
    }
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
    let rt = tokio::runtime::Runtime::new().expect("tokio runtime");
    let mut pool: Option<SqlitePool> = None;
    let mut clock = ThoughtClock::default();
    let mut runs = RunScope::default();
    let mut sequence = first_sequence();
    for msg in rx {
        if pool.is_none() {
            pool = io.open_pool(&rt);
        }
        // A card whose continuation state changed here; its snapshot follows the event.
        let mut card = None;
        let (thread_id, provider, mut ev) = match msg {
            BusMsg::Event(thread_id, provider, ev) => (thread_id, provider, ev),
            // The question becomes an ordinary `UserMessage` once its run is
            // written (a continuation's, `Continued`); a failure writes
            // nothing and the caller does not send.
            BusMsg::Begin { intent, reply } => {
                let begun = match &pool {
                    Some(p) => rt.block_on(store::begin_run(p, &intent)),
                    None => Err("the database is not open".into()),
                };
                let _ = reply.send(begun.clone());
                let Ok(run) = begun else { continue };
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
                (intent.thread_id, intent.provider, ev)
            }
            BusMsg::Release(thread_id, provider) => {
                let next = queue.lock().unwrap().next(thread_id);
                if let Some((msg, opts)) = next {
                    io.dispatch(thread_id, provider, msg, opts);
                }
                continue;
            }
        };
        match io.clock_time() {
            Some(now) => clock.observe_at(thread_id, &mut ev, now),
            None => clock.observe(thread_id, &mut ev),
        }
        // A terminal for no open turn (a repeated `result`) settles nothing
        // and must not release a queued message a second time.
        if thread_id > 0 && runs.is_stray_terminal(thread_id, &ev, &queue) {
            continue;
        }
        let mut item_id = None;
        let mut entity_id = None;
        let tags = runs.enter(thread_id, &ev);
        if let Some(p) = &pool {
            if thread_id > 0 {
                let owner = runs.owner(thread_id, &ev);
                match rt.block_on(store::apply_event(p, thread_id, &ev, owner.as_ref())) {
                    Ok(stored) => {
                        item_id = stored.item_id;
                        if stored.inserted && owner.is_some() {
                            runs.written(thread_id);
                        }
                        entity_id = stored.entity;
                    }
                    // A row that could not be saved is not shown as if it
                    // were: the run gets one error row and a gap instead.
                    Err(e) if makes_row(&ev) => {
                        elog!("[oculus] harness store: {e}");
                        if !runs.first_storage_failure(thread_id) {
                            continue;
                        }
                        ev = HarnessEvent::error(STORAGE_FAILED);
                        let gap = HarnessEvent::DeliveryGap {
                            source: None,
                            cause: GapCause::StorageFailure,
                        };
                        let owner = runs.owner(thread_id, &ev);
                        for event in [&gap, &ev] {
                            match rt.block_on(store::apply(p, thread_id, event, owner.as_ref())) {
                                Ok(id) => item_id = id,
                                Err(e) => elog!("[oculus] harness store: {e}"),
                            }
                        }
                        if item_id.is_some() {
                            runs.written(thread_id);
                        }
                    }
                    Err(e) => elog!("[oculus] harness store: {e}"),
                }
                for (event, id) in runs.after(&rt, p, thread_id, &ev) {
                    io.emit(Envelope {
                        thread_id,
                        provider,
                        item_id: id,
                        run_id: tags.run_id,
                        attempt_id: tags.attempt_id,
                        entity_id: None,
                        sequence: next_sequence(&mut sequence),
                        event,
                    });
                }
            }
            if let Err(e) = rt.block_on(store::save_rate_limits(p, provider, &ev)) {
                elog!("[oculus] harness rate limits: {e}");
            }
            // A finished first exchange is when a thread is named. The
            // claim is atomic (asks at most once); the naming turn runs
            // off this loop, which every thread's events wait behind.
            if thread_id > 0
                && matches!(&ev, HarnessEvent::TurnFinished { status } if status == "completed")
            {
                match rt.block_on(store::claim_naming(p, thread_id)) {
                    Ok(Some(seed)) => {
                        let chain = rt.block_on(jobs::chain(p, jobs::Job::ThreadNaming));
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
            queue.lock().unwrap().hold(thread_id);
        }
        // A closed turn releases the next queued message; its send is
        // spawned, since a send may start a process.
        if thread_id > 0 && matches!(&ev, HarnessEvent::TurnFinished { .. }) {
            let next = queue.lock().unwrap().next(thread_id);
            if let Some((msg, opts)) = next {
                io.dispatch(thread_id, provider, msg, opts);
            }
        }

        // A settled card with a follow-up claims its one continuation: it
        // goes out now on an idle thread, or waits its turn like a message.
        if let (HarnessEvent::CardSettled { proposal }, Some(p)) = (&ev, &pool) {
            match rt.block_on(super::continuation::claim(p, thread_id, proposal)) {
                Ok(Some(line)) => {
                    card = Some(proposal.clone());
                    let opts = SendOptions {
                        continuation: Some(proposal.clone()),
                        ..SendOptions::default()
                    };
                    let mut q = queue.lock().unwrap();
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
            sequence: next_sequence(&mut sequence),
            event: ev,
        });
        if let (Some(id), Some(p)) = (card, &pool) {
            match rt.block_on(crate::db::proposals::get(p, &id)) {
                Ok(proposal) => io.emit(Envelope {
                    thread_id,
                    provider,
                    item_id: None,
                    run_id: None,
                    attempt_id: None,
                    entity_id: None,
                    sequence: next_sequence(&mut sequence),
                    event: HarnessEvent::Proposal { proposal },
                }),
                Err(e) => elog!("[oculus] card continuation {id}: {e}"),
            }
        }
    }
}

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
