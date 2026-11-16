//! Claude 2.1.325 steering, replayed from the stage 4 probes: a user
//! message with `priority: "now"` and a `uuid`, echoed by `command_lifecycle`
//! frames; two `result` frames per steer and one settlement; a foreground
//! tool that runs to its end; text a steer cut off kept as work; and a steer
//! written as the turn's result arrived, which the CLI ran next.

use serde_json::Value;

use super::recorded::drive;

fn by_id(items: &[Value], id: Option<i64>) -> &Value {
    items
        .iter()
        .find(|i| i["id"].as_i64() == id)
        .expect("the row")
}

/// One run settled once, completed, with `answer`, the steer delivered
/// inside it and no turn of the CLI's own.
fn one_run(name: &str, answer: &str) -> (Vec<Value>, Value) {
    let (replay, written) = drive(name, |_| {});
    let runs = replay.ledger["runs"].as_array().unwrap();
    let ran: Vec<&Value> = runs
        .iter()
        .filter(|r| r["execution_status"] != "steered")
        .collect();
    assert_eq!(ran.len(), 1, "{name}: one run");
    let run = ran[0].clone();
    assert_eq!(run["execution_status"], "completed", "{name}");
    assert_eq!(
        by_id(&replay.items, run["answer_item_id"].as_i64())["content"],
        answer,
        "{name}"
    );
    let settled = replay
        .envelopes
        .iter()
        .filter(|e| e["event"]["type"] == "run_settled")
        .count();
    assert_eq!(settled, 1, "{name}: two results, one settlement");
    assert!(replay.items.iter().all(|i| i["kind"] != "unprompted"));
    let steer = by_id(&replay.items, Some(written[0].item_id));
    assert_eq!(steer["run_id"], run["id"]);
    let meta: Value = serde_json::from_str(steer["meta"].as_str().unwrap()).unwrap();
    assert_eq!(meta["delivery"], "delivered", "{name}: echoed");
    (replay.items, run)
}

#[test]
fn a_steer_during_a_foreground_tool_waits_for_it_and_the_run_settles_once() {
    let (items, run) = one_run("claude-steer-tool", "STEER-TOOL-OK");
    // The tool was not cut short: it ran its 20 s and finished.
    let tool = items.iter().find(|i| i["kind"] == "tool").unwrap();
    let meta: Value = serde_json::from_str(tool["meta"].as_str().unwrap()).unwrap();
    assert_eq!(meta["ok"], true);
    assert_eq!(tool["run_id"], run["id"]);
    let early = items
        .iter()
        .find(|i| i["content"] == "BEFORE-SLEEP")
        .unwrap();
    assert_eq!(early["display_role"], "commentary");
}

#[test]
fn a_steer_while_text_streams_keeps_the_cut_text_as_work() {
    let (items, _) = one_run("claude-steer-text", "STEER-TEXT-OK");
    let cut = items
        .iter()
        .find(|i| i["kind"] == "assistant" && i["content"] == "1")
        .expect("the text before the steer stays");
    assert_eq!(cut["display_role"], "commentary");
}

#[test]
fn a_steer_written_as_the_result_arrived_is_answered_in_the_same_run() {
    let (items, _) = one_run("claude-steer-finish", "AFTER-FINISH-OK");
    let first = items.iter().find(|i| i["content"] == "FIRST-DONE").unwrap();
    assert_eq!(first["display_role"], "commentary");
}
