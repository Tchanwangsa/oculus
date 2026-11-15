//! Changing provider mid-chat: what each run records of its selection, the
//! one current binding per provider, the transition a send makes and the
//! context handoff its first prompt carries.

use sqlx::Row;

use crate::harness::core::load;
use crate::harness::SendOptions;

use super::rig::Rig;
use super::scenario::Scenario;

fn options(model: &str, effort: Option<&str>) -> SendOptions {
    SendOptions {
        model: Some(model.into()),
        reasoning_effort: effort.map(String::from),
        ..SendOptions::default()
    }
}

/// A run keeps the model and reasoning level it was sent with and the
/// model its CLI reported; the thread keeps the student's pick.
#[test]
fn a_run_records_what_it_was_sent_with_and_what_its_cli_reported() {
    let mut rig = Rig::start(Scenario::recorded("claude-ls"));
    let run = rig
        .send_with("replay-send", &options("sonnet", Some("high")))
        .unwrap();
    rig.resume();
    rig.close();
    rig.drain();
    let row = rig
        .runtime
        .block_on(
            sqlx::query("SELECT model, effort, resolved_model FROM harness_runs WHERE id = ?1")
                .bind(run.run_id)
                .fetch_one(&rig.pool),
        )
        .unwrap();
    assert_eq!(
        row.get::<Option<String>, _>("model").as_deref(),
        Some("sonnet")
    );
    assert_eq!(
        row.get::<Option<String>, _>("effort").as_deref(),
        Some("high")
    );
    let reported: Option<String> = row.get("resolved_model");
    let reported = reported.expect("the recording's init line names its model");
    assert_ne!(reported, "sonnet");
    let thread_model: Option<String> = rig
        .runtime
        .block_on(
            sqlx::query_scalar("SELECT model FROM harness_threads WHERE id = ?1")
                .bind(rig.thread)
                .fetch_one(&rig.pool),
        )
        .unwrap();
    assert_eq!(
        thread_model.as_deref(),
        Some("sonnet"),
        "the student's pick"
    );

    // A reload reads the same off the run.
    let snapshot = rig
        .runtime
        .block_on(load::read(&rig.pool, rig.thread))
        .unwrap();
    let loaded = serde_json::to_value(&snapshot.runs[0]).unwrap();
    assert_eq!(loaded["provider"], "claude");
    assert_eq!(loaded["model"], "sonnet");
    assert_eq!(loaded["effort"], "high");
    assert_eq!(loaded["resolved_model"], reported.as_str());

    // And a live view is told once, under the run.
    let told = rig.envelopes();
    let selected: Vec<_> = told
        .iter()
        .filter(|e| e["event"]["type"] == "run_selected")
        .collect();
    assert_eq!(selected.len(), 1);
    assert_eq!(selected[0]["runId"], run.run_id);
    assert_eq!(selected[0]["event"]["provider"], "claude");
    assert_eq!(selected[0]["event"]["model"], "sonnet");
    assert_eq!(selected[0]["event"]["effort"], "high");
    rig.finish();
}
