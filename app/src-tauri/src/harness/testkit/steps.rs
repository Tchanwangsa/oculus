//! The rig's own steps, checked against a recording.

use crate::harness::SendOptions;

use super::rig::Rig;
use super::scenario::Scenario;

#[test]
fn a_gate_holds_the_turn_open_until_the_transcript_resumes() {
    let scenario =
        Scenario::recorded("codex-ls").gate("terminal", |f| f["method"] == "turn/completed");
    let mut rig = Rig::start(scenario);
    rig.open();
    rig.until("terminal");
    let status: String = rig
        .runtime
        .block_on(
            sqlx::query_scalar("SELECT execution_status FROM harness_runs").fetch_one(&rig.pool),
        )
        .unwrap();
    assert_eq!(status, "running", "paused mid-turn");
    assert!(rig.queue.lock().unwrap().is_busy(rig.thread));

    rig.resume();
    rig.close();
    let replay = rig.finish();
    assert_eq!(replay.ledger["runs"][0]["execution_status"], "completed");
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
