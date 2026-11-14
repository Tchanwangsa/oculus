//! A CLI that ignores Stop: after fifteen seconds the run is told as
//! unresponsive, and Force stop kills what answers it. Claude and
//! Antigravity lose one chat's process; Codex and opencode lose the server
//! every chat of theirs is answered by.

use serde_json::{json, Value};

use crate::harness::core::{Command, Kind};
use crate::harness::{store, SendOptions};

use super::identity::as_turn;
use super::rig::Rig;
use super::scenario::Scenario;

fn command(rig: &mut Rig, id: &str, thread: i64, kind: Kind) {
    let done = rig.command(Command {
        id: id.into(),
        thread_id: Some(thread),
        kind,
    });
    done.unwrap();
}

fn send(rig: &mut Rig, id: &str, thread: i64) {
    let kind = Kind::Send {
        provider: rig.provider,
        text: "Replay request".into(),
        options: SendOptions::default(),
    };
    command(rig, id, thread, kind);
}

/// The thread's only run that has a question: its id and status.
fn run_of(rig: &Rig, thread: i64) -> (i64, String) {
    rig.runtime
        .block_on(
            sqlx::query_as(
                "SELECT id, execution_status FROM harness_runs
                  WHERE thread_id = ?1 AND initiating_item_id IS NOT NULL",
            )
            .bind(thread)
            .fetch_one(&rig.pool),
        )
        .unwrap()
}

fn unresponsive(rig: &Rig, run_id: i64) -> Option<Value> {
    // The rig's clock jumps to the consumer's next wake whenever the bus is
    // idle, so the fifteen seconds pass at the first drain after the stop.
    rig.drain();
    rig.envelopes()
        .into_iter()
        .find(|e| e["event"]["type"] == "stop_unresponsive" && e["event"]["run_id"] == run_id)
}

/// Claude runs a process per chat. One chat's CLI ignores Stop; it reaches
/// Force stop, which kills that process and ends its run as interrupted
/// with the text it had, while another chat goes on streaming to its end.
#[test]
fn a_cli_that_ignores_stop_is_force_stopped_while_another_chat_keeps_streaming() {
    let mut scenario = Scenario::recorded("claude-ls").gate("terminal", |f| f["type"] == "result");
    let other = scenario.fixture.expected_outbound[0].clone();
    let mut interrupt = scenario.fixture.expected_outbound[0].clone();
    interrupt.frame = json!({
        "type": "control_request", "request_id": "oculus-1",
        "request": {"subtype": "interrupt"},
    });
    scenario
        .fixture
        .expected_outbound
        .extend([other, interrupt]);
    let frames = scenario.frames.clone();
    let ends = frames.iter().position(|f| f["type"] == "result").unwrap();
    let mut rig = Rig::start(scenario);
    rig.open();
    rig.until("terminal");
    let stuck = run_of(&rig, rig.thread).0;

    // A second chat is mid-answer on its own process.
    let (other, mut streaming) = rig.second_chat();
    send(&mut rig, "other", other);
    for frame in &frames[..ends] {
        (streaming.inbound)(frame);
    }
    rig.drain();
    assert_eq!(run_of(&rig, other).1, "running");

    let thread = rig.thread;
    let stop = Kind::Stop {
        run_id: Some(stuck),
    };
    command(&mut rig, "stop", thread, stop);
    let told = unresponsive(&rig, stuck).expect("the CLI said nothing for fifteen seconds");
    assert_eq!(told["event"]["others"], 0, "a process of its own");
    assert_eq!(run_of(&rig, thread).1, "running", "still open: only told");
    assert!(rig.alive());

    command(&mut rig, "force", thread, Kind::ForceStop { run_id: stuck });
    rig.drain();
    assert!(!rig.alive(), "its process was killed");
    assert_eq!(run_of(&rig, thread).1, "interrupted");
    assert!(streaming.handle.is_alive(), "the other chat's was not");
    assert_eq!(run_of(&rig, other).1, "running");

    // What the killed process still says reaches nobody, and the other
    // chat's answer arrives whole.
    rig.resume();
    for frame in &frames[ends..] {
        (streaming.inbound)(frame);
    }
    rig.drain();
    assert_eq!(run_of(&rig, thread).1, "interrupted");
    assert_eq!(run_of(&rig, other).1, "completed");
    let replay = rig.finish();
    let ended = replay
        .envelopes
        .iter()
        .filter(|e| e["event"]["type"] == "turn_finished" && e["threadId"] == thread)
        .count();
    assert_eq!(ended, 1, "the stopped run ended once");
    let partial = replay
        .items
        .iter()
        .filter(|i| i["kind"] == "assistant")
        .count();
    assert!(partial > 0, "what it had said is kept");
}

/// Codex answers every chat from one server. Force stop of one chat's run
/// ends the other chat's turn too, and says so beforehand; both settle as
/// interrupted, and what waited behind either stays, held.
#[test]
fn force_stop_on_a_shared_server_settles_every_chat_on_it_as_interrupted() {
    const FIRST: &str = "01a08b4a-5f68-7c42-bc82-ab8e70d37d01";
    let mut scenario =
        Scenario::recorded("codex-ls").gate("terminal", |f| f["method"] == "turn/completed");
    let native = scenario.fixture.native_session.clone();
    let mut other = scenario.fixture.expected_outbound[0].clone();
    other.response["turn"]["id"] = "other-turn".into();
    let mut interrupt = scenario.fixture.expected_outbound[0].clone();
    interrupt.frame = json!({
        "jsonrpc": "2.0", "id": 2, "method": "turn/interrupt",
        "params": {"threadId": native, "turnId": FIRST},
    });
    interrupt.response = json!({});
    scenario
        .fixture
        .expected_outbound
        .extend([other, interrupt]);
    let frames = scenario.frames.clone();
    let ends = frames
        .iter()
        .position(|f| f["method"] == "turn/completed")
        .unwrap();
    let mut rig = Rig::start(scenario);
    rig.open();
    rig.until("terminal");
    let thread = rig.thread;
    let stuck = run_of(&rig, thread).0;

    let (other, mut streaming) = rig.second_chat();
    send(&mut rig, "other", other);
    for frame in &as_turn(&frames, FIRST, "other-turn")[..ends] {
        (streaming.inbound)(frame);
    }
    rig.drain();
    let cut = run_of(&rig, other).0;
    send(&mut rig, "behind", other);

    let stop = Kind::Stop {
        run_id: Some(stuck),
    };
    command(&mut rig, "stop", thread, stop);
    let told = unresponsive(&rig, stuck).expect("the CLI said nothing for fifteen seconds");
    assert_eq!(told["event"]["others"], 1, "one other chat has a run there");

    command(&mut rig, "force", thread, Kind::ForceStop { run_id: stuck });
    rig.drain();
    assert_eq!(run_of(&rig, thread).1, "interrupted");
    assert_eq!(run_of(&rig, other), (cut, "interrupted".to_string()));
    assert!(!rig.alive() && !streaming.handle.is_alive());
    let behind = rig.runtime.block_on(store::queue::list(&rig.pool, other));
    let behind = behind.unwrap();
    assert_eq!(behind.len(), 1);
    assert!(
        behind[0].held,
        "the other chat's waiting message is not sent into this"
    );

    // Force stop again, or for a run that is over, kills nothing more.
    command(
        &mut rig,
        "force-again",
        thread,
        Kind::ForceStop { run_id: stuck },
    );
    let replay = rig.finish();
    let stops: Vec<_> = replay
        .envelopes
        .iter()
        .filter(|e| e["event"]["type"] == "turn_finished")
        .map(|e| e["event"]["status"].as_str().unwrap())
        .collect();
    assert_eq!(stops, ["interrupted", "interrupted"]);
}
