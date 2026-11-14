//! Events say whose they are: a run is settled only by its own turn's end,
//! and a session that has ended or been replaced is heard no more. Each
//! test replays what goes wrong when an event is taken for the open run's
//! only because it arrived while that run was open.

use std::sync::{Arc, Mutex};

use serde_json::Value;
use sqlx::SqlitePool;

use crate::harness::app::consumer::{consume, ConsumerIo};
use crate::harness::app::{BusMsg, Envelope};
use crate::harness::core::effect::Effect;
use crate::harness::{jobs, store, HarnessEvent, OwnTurns, Provider, Release, SendOptions};

use super::rig::Rig;
use super::scenario::Scenario;

/// The recording's frames as another turn of the same thread would send them.
pub(super) fn as_turn(frames: &[Value], from: &str, to: &str) -> Vec<Value> {
    frames
        .iter()
        .map(|f| serde_json::from_str(&f.to_string().replace(from, to)).unwrap())
        .collect()
}

fn status_of(rig: &Rig, run: i64) -> String {
    rig.runtime
        .block_on(
            sqlx::query_scalar("SELECT execution_status FROM harness_runs WHERE id = ?1")
                .bind(run)
                .fetch_one(&rig.pool),
        )
        .unwrap()
}

/// Codex ends a turn twice, or ends one it ran itself, while the next
/// question is being answered: that run is not the one that ended.
#[test]
fn a_second_turn_completed_does_not_close_the_next_run() {
    const FIRST: &str = "01a08b4a-5f68-7c42-bc82-ab8e70d37d01";
    let mut scenario = Scenario::recorded("codex-ls");
    let mut again = scenario.fixture.expected_outbound[0].clone();
    again.frame["id"] = 2.into();
    again.response["turn"]["id"] = "second-turn".into();
    scenario.fixture.expected_outbound.push(again);
    let frames = scenario.frames.clone();
    let ended = frames
        .iter()
        .find(|f| f["method"] == "turn/completed")
        .unwrap()
        .clone();
    let mut rig = Rig::start(scenario);
    rig.open();
    rig.resume();
    rig.drain();

    let second = rig.send("second").unwrap();
    rig.feed(&ended);
    rig.drain();
    assert_eq!(status_of(&rig, second.run_id), "running");
    assert!(rig.busy(), "nothing queued behind the run is let go");

    for frame in as_turn(&frames, FIRST, "second-turn") {
        rig.feed(&frame);
    }
    rig.eof();
    let replay = rig.finish();
    let runs = replay.ledger["runs"].as_array().unwrap();
    assert_eq!(runs[1]["execution_status"], "completed", "{runs:?}");
    let answer = replay
        .items
        .iter()
        .find(|i| i["id"] == runs[1]["answer_item_id"])
        .expect("the second run kept its own answer");
    assert_eq!(answer["run_id"], second.run_id);
    let terminals = replay
        .envelopes
        .iter()
        .filter(|e| e["event"]["type"] == "turn_finished")
        .count();
    assert_eq!(terminals, 2, "one per run; the repeat reached nobody");
    assert!(!replay.busy);
}

/// A process the manager replaced keeps the sink it was spawned with. What
/// it says later (here, the turn it starts when a background task finishes)
/// must not reach the run the new session is answering.
#[test]
fn a_replaced_session_is_heard_no_more() {
    let mut scenario = Scenario::recorded("claude-ls");
    let again = scenario.fixture.expected_outbound[0].clone();
    scenario.fixture.expected_outbound.push(again);
    let frames = scenario.frames.clone();
    let mut rig = Rig::start(scenario);
    rig.open();
    rig.resume();
    rig.drain();

    // A different reasoning level needs a new process.
    let mut replaced = rig.next_session();
    let opts = SendOptions {
        reasoning_effort: Some("high".into()),
        ..SendOptions::default()
    };
    let second = rig.send_with("second", &opts).unwrap();
    let answer = frames
        .iter()
        .position(|f| f["type"] == "assistant")
        .unwrap();
    for frame in &frames[..answer] {
        rig.feed(frame);
    }
    // The old process starts a turn of its own and ends it.
    for frame in &frames {
        if frame["type"] == "stream_event" || frame["type"] == "result" {
            (replaced.inbound)(frame);
        }
    }
    for frame in &frames[answer..] {
        rig.feed(frame);
    }
    rig.eof();
    let replay = rig.finish();

    let runs = replay.ledger["runs"].as_array().unwrap();
    assert_eq!(runs.len(), 2);
    assert_eq!(runs[1]["id"], second.run_id);
    assert_eq!(runs[1]["execution_status"], "completed", "{runs:?}");
    assert!(
        !replay.items.iter().any(|i| i["kind"] == "unprompted"),
        "the old process's own turn is not this thread's"
    );
    let after: Vec<_> = replay
        .items
        .iter()
        .filter(|i| i["id"].as_i64().unwrap() > second.item_id)
        .collect();
    assert!(!after.is_empty());
    for item in after {
        assert_eq!(item["run_id"], second.run_id, "{item}");
    }
    assert!(!replay.busy);
}

/// A different reasoning level needs a new process, and the old one must not
/// outlive the swap: left running, it would resume the same session id
/// beside its successor.
#[test]
fn changing_the_reasoning_level_leaves_one_live_process() {
    for name in ["claude-ls", "antigravity-ls"] {
        let mut scenario = Scenario::recorded(name);
        let again = scenario.fixture.expected_outbound[0].clone();
        scenario.fixture.expected_outbound.push(again);
        let mut rig = Rig::start(scenario);
        rig.open();
        rig.resume();
        rig.drain();

        let replaced = rig.next_session();
        assert!(replaced.handle.is_alive(), "{name}");
        let opts = SendOptions {
            reasoning_effort: Some("high".into()),
            ..SendOptions::default()
        };
        rig.send_with("second", &opts).unwrap();
        assert!(
            !replaced.handle.is_alive(),
            "{name}: the old process was killed"
        );
        assert_eq!(rig.harness.live_threads(rig.provider), vec![rig.thread]);
        rig.finish();
    }
}

/// The process is killed mid-answer. The run ends there, interrupted, with
/// what had streamed kept; whatever the dying process still says, even after
/// the next question has started, reaches no run.
#[test]
fn a_killed_processs_late_terminal_reaches_no_run() {
    let mut scenario = Scenario::recorded("claude-ls");
    let again = scenario.fixture.expected_outbound[0].clone();
    scenario.fixture.expected_outbound.push(again);
    let frames = scenario.frames.clone();
    let answer = frames
        .iter()
        .rposition(|f| f["type"] == "assistant")
        .unwrap();
    let last = frames[answer].clone();
    let mut rig = Rig::start(scenario.gate("answer", move |f| *f == last));
    rig.open();
    rig.until("answer");
    rig.harness.release(rig.thread, Release::Stopped);
    rig.drain();
    assert!(!rig.busy(), "the stopped run let go of the thread");

    let mut killed = rig.next_session();
    let second = rig.send("second").unwrap();
    for frame in &frames[answer..] {
        (killed.inbound)(frame);
    }
    (killed.eof)();
    rig.drain();
    assert_eq!(status_of(&rig, second.run_id), "running");

    for frame in &frames {
        rig.feed(frame);
    }
    rig.eof();
    let replay = rig.finish();
    let runs = replay.ledger["runs"].as_array().unwrap();
    assert_eq!(runs[0]["execution_status"], "interrupted", "{runs:?}");
    assert_eq!(runs[1]["execution_status"], "completed", "{runs:?}");
    let partial: Vec<_> = replay
        .items
        .iter()
        .filter(|i| i["lifecycle"] == "partial")
        .collect();
    assert_eq!(partial.len(), 1, "what streamed before the kill is kept");
    assert_eq!(partial[0]["run_id"], runs[0]["id"]);
    assert!(!partial[0]["content"].as_str().unwrap().is_empty());
    assert!(
        !replay.items.iter().any(|i| i["kind"] == "error"),
        "the dying process's exit is nobody's error"
    );
    let terminals = replay
        .envelopes
        .iter()
        .filter(|e| e["event"]["type"] == "turn_finished")
        .count();
    assert_eq!(terminals, 2, "one per run");
    assert!(!replay.busy);
}

/// opencode's server dies after taking the prompt and before the turn's
/// first event: its bridge has no turn open to fail, so the consumer does.
#[test]
fn a_process_that_dies_before_its_turn_opens_fails_the_turn() {
    let mut scenario = Scenario::recorded("opencode-v1-text");
    scenario.frames.clear();
    let replay = super::replay::run(scenario);
    let runs = replay.ledger["runs"].as_array().unwrap();
    assert_eq!(runs[0]["execution_status"], "failed", "{runs:?}");
    assert!(replay
        .items
        .iter()
        .any(|i| i["kind"] == "error" && i["run_id"] == runs[0]["id"]));
    assert!(!replay.busy, "nothing waits behind a dead turn");
}

/// Records envelopes; nothing is named or sent.
struct Recorder {
    pool: SqlitePool,
    events: Vec<Value>,
}

impl ConsumerIo for Recorder {
    fn open_pool(&mut self, _: &tokio::runtime::Runtime) -> Option<SqlitePool> {
        Some(self.pool.clone())
    }
    fn name(&mut self, _: i64, _: Provider, _: Vec<jobs::JobSelection>, _: store::NamingSeed) {}
    fn perform(&mut self, _: Effect) {}
    fn emit(&mut self, envelope: Envelope) {
        self.events.push(serde_json::to_value(envelope).unwrap());
    }
}

fn from_session(thread: i64, generation: u64, event: HarnessEvent) -> BusMsg {
    BusMsg::Event {
        thread_id: thread,
        provider: Provider::Claude,
        generation: Some(generation),
        event,
    }
}

fn said(text: &str) -> HarnessEvent {
    HarnessEvent::AssistantMessage {
        text: text.into(),
        source: None,
    }
}

fn ended(turn: &str) -> HarnessEvent {
    HarnessEvent::TurnFinished {
        status: "completed".into(),
        turn: Some(turn.into()),
    }
}

/// Claude picks up a finished background task just as a message goes in: the
/// CLI runs its own turn first, then the student's. The run waits for its
/// own turn, and the CLI's turn owns none of its rows.
#[test]
fn a_send_racing_a_turn_the_cli_starts_is_settled_by_its_own_turn() {
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let (_dir, pool) = runtime.block_on(super::runs::scratch_pool("turn-race"));
    let thread = runtime
        .block_on(store::create_thread(
            &pool,
            Provider::Claude,
            None,
            None,
            None,
            "q",
        ))
        .unwrap();
    let (send, begun) = super::runs::send(thread, Provider::Claude, "race");
    let started = |turn: &str| HarnessEvent::TurnStarted {
        turn: Some(turn.into()),
    };
    let bus = vec![
        send,
        BusMsg::Bound {
            thread_id: thread,
            generation: 1,
            turn: "1".into(),
        },
        from_session(
            thread,
            1,
            HarnessEvent::UnpromptedTurn {
                after: vec!["Index the notes".into()],
            },
        ),
        from_session(thread, 1, started("2")),
        from_session(thread, 1, said("The index is ready.")),
        from_session(thread, 1, ended("2")),
        from_session(thread, 1, started("1")),
        from_session(thread, 1, said("Here is the answer.")),
        from_session(thread, 1, ended("1")),
    ];
    let queue = Arc::new(Mutex::new(OwnTurns::default()));
    let mut io = Recorder {
        pool: pool.clone(),
        events: Vec::new(),
    };
    consume(bus, queue.clone(), &mut io);
    let run = runtime.block_on(begun).unwrap().unwrap().run.unwrap();

    let ledger = runtime.block_on(super::snapshot::ledger(&pool, thread));
    assert_eq!(ledger["runs"][0]["execution_status"], "completed");
    let items = runtime.block_on(super::snapshot::items(&pool, thread));
    let row = |text: &str| items.iter().find(|i| i["content"] == text).unwrap();
    assert_eq!(row("Here is the answer.")["run_id"], run.run_id);
    assert_eq!(
        ledger["runs"][0]["answer_item_id"],
        row("Here is the answer.")["id"]
    );
    assert!(
        row("The index is ready.")["run_id"].is_null(),
        "the CLI's own turn owns no run"
    );
    let settled: Vec<_> = io
        .events
        .iter()
        .filter(|e| e["event"]["type"] == "run_settled")
        .collect();
    assert_eq!(settled.len(), 1);
    assert!(
        !queue.lock().unwrap().is_held(thread)
            && runtime
                .block_on(store::queue::open(&pool, thread))
                .unwrap()
                .is_none(),
        "both turns let go of the thread"
    );
    runtime.block_on(pool.close());
}

fn delta(text: &str) -> HarnessEvent {
    HarnessEvent::AssistantDelta {
        text: text.into(),
        item: None,
    }
}

/// One run on a scratch thread, bound to turn `1` of session `1`, then `bus`.
fn one_run(
    name: &str,
    bus: impl FnOnce(i64) -> Vec<BusMsg>,
) -> (Value, Vec<Value>, Vec<Value>, bool) {
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let (_dir, pool) = runtime.block_on(super::runs::scratch_pool(name));
    let thread = runtime
        .block_on(store::create_thread(
            &pool,
            Provider::Claude,
            None,
            None,
            None,
            "q",
        ))
        .unwrap();
    let (send, _begun) = super::runs::send(thread, Provider::Claude, name);
    let mut messages = vec![
        send,
        BusMsg::Bound {
            thread_id: thread,
            generation: 1,
            turn: "1".into(),
        },
    ];
    messages.extend(bus(thread));
    let queue = Arc::new(Mutex::new(OwnTurns::default()));
    let mut io = Recorder {
        pool: pool.clone(),
        events: Vec::new(),
    };
    consume(messages, queue.clone(), &mut io);
    let ledger = runtime.block_on(super::snapshot::ledger(&pool, thread));
    let items = runtime.block_on(super::snapshot::items(&pool, thread));
    let open = runtime.block_on(store::queue::open(&pool, thread));
    let busy = queue.lock().unwrap().is_held(thread) || open.unwrap().is_some();
    runtime.block_on(pool.close());
    (ledger, items, io.events, busy)
}

fn released(thread: i64, reason: Release) -> BusMsg {
    BusMsg::Released {
        thread_id: thread,
        provider: Provider::Claude,
        generation: 1,
        reason,
    }
}

/// One rule, whichever bridge: a session that ends with its turn open fails
/// that turn, and what had streamed is kept as a partial reply with its gap.
#[test]
fn a_process_that_dies_mid_answer_fails_its_turn_with_the_text_kept() {
    let (ledger, items, events, busy) = one_run("turn-died", |thread| {
        vec![
            from_session(thread, 1, delta("Half an ans")),
            from_session(thread, 1, HarnessEvent::Exited { code: Some(1) }),
            // Nothing more is heard from it, not even a second exit.
            from_session(thread, 1, said("late")),
            from_session(thread, 1, HarnessEvent::Exited { code: Some(1) }),
        ]
    });
    let run = &ledger["runs"][0];
    assert_eq!(run["execution_status"], "failed");
    assert_eq!(run["delivery_state"], "incomplete");
    let kept = items.iter().find(|i| i["kind"] == "assistant").unwrap();
    assert_eq!(kept["content"], "Half an ans");
    assert_eq!(kept["lifecycle"], "partial");
    assert_eq!(kept["run_id"], run["id"]);
    assert_eq!(ledger["gaps"][0]["cause"], "missing_snapshot");
    assert_eq!(items.iter().filter(|i| i["kind"] == "error").count(), 1);
    assert!(!items.iter().any(|i| i["content"] == "late"));
    let kinds: Vec<_> = events
        .iter()
        .map(|e| e["event"]["type"].as_str().unwrap())
        .collect();
    assert_eq!(
        kinds.iter().filter(|k| **k == "turn_finished").count(),
        1,
        "{kinds:?}"
    );
    assert_eq!(kinds.iter().filter(|k| **k == "exited").count(), 1);
    assert!(!busy);
}

/// The reason a session ended decides what becomes of its open turn.
#[test]
fn a_release_closes_the_open_turn_by_its_reason() {
    for (reason, status) in [
        (Release::Stopped, "interrupted"),
        (Release::Replaced, "interrupted"),
        (Release::Exited, "failed"),
    ] {
        let (ledger, items, _, busy) = one_run("turn-released", |thread| {
            vec![
                from_session(thread, 1, delta("So far")),
                released(thread, reason),
                from_session(thread, 1, ended("1")),
            ]
        });
        assert_eq!(ledger["runs"][0]["execution_status"], status, "{reason:?}");
        assert!(
            items
                .iter()
                .any(|i| i["content"] == "So far" && i["lifecycle"] == "partial"),
            "{reason:?}"
        );
        assert!(!busy, "{reason:?}");
    }
    // A quit writes nothing: the next startup repairs the run.
    let (ledger, _, events, busy) = one_run("turn-quit", |thread| {
        vec![
            from_session(thread, 1, delta("So far")),
            released(thread, Release::Quit),
            from_session(thread, 1, said("late")),
        ]
    });
    assert_eq!(ledger["runs"][0]["execution_status"], "starting");
    assert!(busy);
    assert!(!events.iter().any(|e| e["event"]["type"] == "turn_finished"));
}

/// Another turn's end arrives while the run's agent is still thinking: it
/// ends nothing here, so it must not stop the clock on that thought either.
#[test]
fn a_stray_terminal_leaves_the_live_thought_timed() {
    let (_, _, events, _) = one_run("turn-stray-thought", |thread| {
        vec![
            from_session(thread, 1, HarnessEvent::ThinkingDelta { text: "hm".into() }),
            from_session(thread, 1, ended("0")),
            from_session(
                thread,
                1,
                HarnessEvent::Thinking {
                    text: "hm".into(),
                    ms: None,
                    source: None,
                },
            ),
            from_session(thread, 1, ended("1")),
        ]
    });
    let thought = events
        .iter()
        .find(|e| e["event"]["type"] == "thinking")
        .expect("the thought went out");
    assert!(thought["event"]["ms"].is_u64(), "{thought}");
}
