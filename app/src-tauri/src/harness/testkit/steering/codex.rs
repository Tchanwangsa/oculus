//! Codex 0.169.0 steering, replayed from the stage 4 probes: `turn/steer`
//! under the turn the `turn/start` reply named, its echo, a command that
//! finishes after the turn's terminal, text a steer cut off, and a steer
//! the server refuses because the turn has ended.

use serde_json::Value;

use crate::harness::manager::policy::Mode;

use super::super::rig::Rig;
use super::super::scenario::Scenario;
use super::recorded::{drive, gates, say};

fn rows<'a>(items: &'a [Value], kind: &str) -> Vec<&'a Value> {
    items.iter().filter(|i| i["kind"] == kind).collect()
}

fn delivery(item: &Value) -> String {
    let meta: Value = serde_json::from_str(item["meta"].as_str().unwrap()).unwrap();
    meta["delivery"].as_str().unwrap().to_string()
}

#[test]
fn a_steer_during_a_command_joins_the_turn_and_the_late_completion_stays_with_its_run() {
    let (replay, written) = drive("codex-steer-tool", |rig| {
        say(rig, "next", "Replay request", Mode::Auto);
        rig.drain();
    });
    let runs = replay.ledger["runs"].as_array().unwrap();
    let first = &runs[0];
    assert_eq!(first["execution_status"], "completed");
    assert_eq!(first["delivery_state"], "delivered");
    let items = &replay.items;
    let answer = items
        .iter()
        .find(|i| Some(i["id"].as_i64().unwrap()) == first["answer_item_id"].as_i64())
        .unwrap();
    assert_eq!(answer["content"], "LIFECYCLE_STEERED");
    let steer = rows(items, "steer")[0];
    assert_eq!(steer["run_id"], first["id"]);
    assert_eq!(steer["id"], written[0].item_id);
    assert_eq!(delivery(steer), "delivered", "the echo came back");
    // The command was not cut short: its completion came after the turn's
    // terminal, after the next message started, and it stays on its row in
    // the first run.
    let tool = rows(items, "tool")[0];
    assert_eq!(tool["run_id"], first["id"]);
    let meta: Value = serde_json::from_str(tool["meta"].as_str().unwrap()).unwrap();
    assert_eq!(meta["ok"], true);
    let second = &runs[2];
    assert!(items
        .iter()
        .filter(|i| i["run_id"] == second["id"])
        .all(|i| i["kind"] != "tool"));
    // Order inside the run: question, commentary, the command, the steer,
    // the answer.
    let kinds: Vec<&str> = items
        .iter()
        .filter(|i| i["run_id"] == first["id"])
        .map(|i| i["kind"].as_str().unwrap())
        .collect();
    assert_eq!(kinds, ["user", "assistant", "tool", "steer", "assistant"]);
    // The steer's identity is the steered row between the two runs that ran.
    assert_eq!(runs[1]["execution_status"], "steered");
    assert_eq!(runs[1]["id"], written[0].message);
}

#[test]
fn a_steer_mid_stream_keeps_the_cut_text_and_one_at_the_terminal_becomes_the_next_run() {
    let (replay, written) = drive("codex-steer-text-finish", |_| {});
    let runs = replay.ledger["runs"].as_array().unwrap();
    let first = &runs[0];
    assert_eq!(first["execution_status"], "completed");
    let items = &replay.items;
    let mine: Vec<&Value> = items
        .iter()
        .filter(|i| i["run_id"] == first["id"])
        .collect();
    // The text the steer cut off never got its `item/completed`: kept as
    // it streamed, partial, as work, with no gap.
    let cut = mine
        .iter()
        .find(|i| i["kind"] == "assistant" && i["lifecycle"] == "partial")
        .expect("the cut text is kept");
    assert!(cut["content"]
        .as_str()
        .unwrap()
        .starts_with("BEFORE_STREAM_STEER"));
    assert_eq!(cut["display_role"], "commentary");
    assert!(replay.ledger["gaps"].as_array().unwrap().is_empty());
    let answer = mine.iter().find(|i| i["display_role"] == "answer").unwrap();
    assert_eq!(answer["content"], "STREAM_STEERED");
    // The steer the server refused (`no active turn`) left the run and
    // went out next as its own run, under its own identity, once.
    let steers = rows(items, "steer");
    assert_eq!(steers.len(), 1, "only the steer that reached the turn");
    let late = written[1];
    let second = runs.iter().find(|r| r["id"] == late.message).unwrap();
    assert_ne!(second["execution_status"], "steered");
    let question = items
        .iter()
        .find(|i| Some(i["id"].as_i64().unwrap()) == second["initiating_item_id"].as_i64())
        .unwrap();
    assert_eq!(question["kind"], "user");
    assert_eq!(question["content"], "FINISH_RACE_MARKER");
    let starts: Vec<&Value> = replay
        .outbound
        .iter()
        .filter(|f| f["method"] == "turn/start")
        .collect();
    assert_eq!(starts.len(), 2);
    assert_eq!(
        starts[1]["params"]["input"][0]["text"],
        "FINISH_RACE_MARKER"
    );
}

#[test]
fn a_steer_saved_and_never_written_waits_held_after_a_restart() {
    let mut scenario = Scenario::recorded("codex-steer-tool");
    // Only the question goes out: the steer is never written.
    scenario.fixture.expected_outbound.truncate(1);
    let frames = scenario.frames.clone();
    let steer = scenario.fixture.settings["steers"][0].clone();
    let at = (0..frames.len())
        .find(|i| gates(&steer, &frames, *i))
        .unwrap();
    let mut rig = Rig::start(scenario);
    rig.open();
    for frame in &frames[..at] {
        rig.feed(frame);
        rig.drain();
    }
    rig.hold_effects();
    let outcome = say(&mut rig, "steer", "correction", Mode::Auto);
    assert!(outcome.steered.is_some());
    let rig = rig.restart();
    rig.startup().unwrap();
    let queued = crate::harness::store::queue::list(&rig.pool, rig.thread);
    let queued = rig.runtime.block_on(queued).unwrap();
    assert_eq!(queued.len(), 1);
    assert_eq!(queued[0].text, "correction");
    assert!(queued[0].held, "nothing starts a turn at launch");
    let steers: i64 = rig
        .runtime
        .block_on(
            sqlx::query_scalar("SELECT COUNT(*) FROM harness_items WHERE kind = 'steer'")
                .fetch_one(&rig.pool),
        )
        .unwrap();
    assert_eq!(steers, 0, "its row went with the turn it never reached");
    assert_eq!(queued[0].id, outcome.steered.unwrap().message.to_string());
    let _ = rig.finish();
}
