//! Events say whose they are: a run is settled only by its own turn's end,
//! and a session that has been replaced is heard no more. Each test replays
//! a hole that was open before turns and sessions had ids.

use std::sync::{Arc, Mutex};

use serde_json::Value;
use sqlx::SqlitePool;

use crate::harness::app::consumer::{consume, ConsumerIo};
use crate::harness::app::{BusMsg, Envelope};
use crate::harness::{jobs, store, HarnessEvent, Provider, Queue, QueuedMessage, SendOptions};

use super::rig::Rig;
use super::scenario::Scenario;

/// The recording's frames as another turn of the same thread would send them.
fn as_turn(frames: &[Value], from: &str, to: &str) -> Vec<Value> {
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
    assert!(
        rig.queue.lock().unwrap().is_busy(rig.thread),
        "nothing queued behind the run is let go"
    );

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
    fn dispatch(&mut self, _: i64, _: Provider, _: QueuedMessage, _: SendOptions) {
        panic!("nothing is queued in this test");
    }
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
    let (reply, begun) = tokio::sync::oneshot::channel();
    let started = |turn: &str| HarnessEvent::TurnStarted {
        turn: Some(turn.into()),
    };
    let bus = vec![
        BusMsg::Begin {
            intent: super::runs::intent(thread, "race", None),
            reply,
        },
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
    let queue = Arc::new(Mutex::new(Queue::default()));
    assert!(queue.lock().unwrap().try_claim(thread));
    let mut io = Recorder {
        pool: pool.clone(),
        events: Vec::new(),
    };
    consume(bus, queue.clone(), &mut io);
    let run = runtime.block_on(begun).unwrap().unwrap();

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
        !queue.lock().unwrap().is_busy(thread),
        "both turns let go of the thread"
    );
    runtime.block_on(pool.close());
}
