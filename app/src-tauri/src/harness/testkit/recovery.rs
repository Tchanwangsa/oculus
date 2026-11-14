//! The write that may not be lost, and the restart after it: a run whose
//! terminal was seen is settled, however long the database refuses.

use super::rig::Rig;
use super::scenario::Scenario;

fn status(rig: &Rig) -> String {
    rig.runtime
        .block_on(
            sqlx::query_scalar("SELECT execution_status FROM harness_runs").fetch_one(&rig.pool),
        )
        .unwrap()
}

/// The database refuses writes as the answer ends. The settle is retried
/// until it lands; the chat stays held meanwhile, and the next startup finds
/// a completed run, not one to relabel as cut off.
#[test]
fn a_settle_that_fails_twice_then_lands_leaves_the_run_completed_after_a_restart() {
    let mut scenario =
        Scenario::recorded("codex-ls").gate("terminal", |f| f["method"] == "turn/completed");
    scenario.restart = true;
    let mut rig = Rig::start(scenario);
    rig.open();
    rig.until("terminal");

    rig.read_only(true);
    let woken = rig.wakes();
    rig.resume();
    // The first try failed with the terminal; each wake since is a retry.
    rig.await_wakes(woken + 2);
    assert_eq!(status(&rig), "running");
    assert!(
        rig.queue.lock().unwrap().is_busy(rig.thread),
        "the chat's next message waits for the settle"
    );

    rig.read_only(false);
    rig.drain();
    assert_eq!(status(&rig), "completed");
    assert!(!rig.queue.lock().unwrap().is_busy(rig.thread));

    rig.close();
    let rig = rig.restart();
    rig.startup().unwrap();
    let replay = rig.finish();
    assert!(replay.restarted);
    let run = &replay.ledger["runs"][0];
    assert_eq!(run["execution_status"], "completed", "{run}");
    assert_eq!(run["delivery_state"], "delivered");
    assert!(
        !replay.items.iter().any(|i| i["kind"] == "delivery_notice"),
        "startup did not relabel a finished answer"
    );
    let count = |kind: &str| {
        replay
            .envelopes
            .iter()
            .filter(|e| e["event"]["type"] == kind)
            .count()
    };
    assert_eq!(count("turn_finished"), 1, "told once, when it had landed");
    assert_eq!(count("run_settled"), 1);
    let (settled, finished) = (replay.envelopes.len() - 2, replay.envelopes.len() - 1);
    assert_eq!(replay.envelopes[settled]["event"]["type"], "run_settled");
    assert_eq!(replay.envelopes[finished]["event"]["type"], "turn_finished");
}

/// The app was killed mid-answer and a message is sent the moment it is back.
/// The send waits at startup's gate: the repair closes the old run first,
/// and never touches the new one.
#[test]
fn a_send_issued_during_startup_waits_for_the_repair() {
    let mut scenario =
        Scenario::recorded("codex-ls").mutate(super::scenario::Mutation::PartialOnly);
    let mut again = scenario.fixture.expected_outbound[0].clone();
    again.response["turn"]["id"] = "after-restart".into();
    scenario.fixture.expected_outbound.push(again);
    let mut rig = Rig::start(scenario);
    rig.open();
    rig.resume();
    rig.drain();
    assert_eq!(status(&rig), "running", "killed mid-answer");

    let mut rig = rig.restart();
    let second = rig.send("after-restart").unwrap();
    rig.drain();
    let replay = rig.finish();
    let runs = replay.ledger["runs"].as_array().unwrap();
    assert_eq!(runs[0]["execution_status"], "interrupted", "{runs:?}");
    assert_eq!(
        runs[1]["execution_status"], "running",
        "the repair was over before the new run began"
    );
    let closed = replay
        .items
        .iter()
        .find(|i| i["kind"] == "delivery_notice")
        .expect("the old run says the app closed under it");
    assert_eq!(closed["run_id"], runs[0]["id"]);
    assert!(
        closed["id"].as_i64().unwrap() < second.item_id,
        "repaired before the question was written"
    );
}
