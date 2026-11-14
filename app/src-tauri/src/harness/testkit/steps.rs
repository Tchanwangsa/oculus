//! The rig's own steps, checked against a recording.

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
    assert!(rig.busy());

    rig.resume();
    rig.close();
    let replay = rig.finish();
    assert_eq!(replay.ledger["runs"][0]["execution_status"], "completed");
    assert!(!replay.busy);
}
