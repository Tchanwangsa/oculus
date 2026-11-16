//! ACP restart steering: recorded cancel frames, derived continuations and
//! races, driven through the real command, manager and session transport.

use serde_json::Value;

use crate::harness::core::{Command, Kind};
use crate::harness::manager::policy::Mode;

use super::super::{replay::Replay, rig::Rig, scenario::Scenario};
use super::recorded::say;

pub(in super::super) const NAMES: [&str; 9] = [
    "antigravity-steer-successive",
    "antigravity-steer-tool",
    "antigravity-steer-text",
    "antigravity-steer-two",
    "antigravity-steer-missed",
    "antigravity-steer-stop",
    "antigravity-steer-during-stop",
    "antigravity-steer-death",
    "antigravity-steer-error",
];

pub(in super::super) fn recorded(name: &str) -> Replay {
    drive(name, |_, _| {})
}

fn drive(name: &str, mut observe: impl FnMut(&mut Rig, usize)) -> Replay {
    let scenario = Scenario::recorded(name);
    let frames = scenario.frames.clone();
    let actions = scenario.fixture.settings["actions"]
        .as_array()
        .unwrap()
        .clone();
    let mut rig = Rig::start(scenario);
    rig.open();
    let mut outstanding = 1;
    let mut sent = 1;
    for (at, frame) in frames.iter().enumerate() {
        for (index, action) in actions
            .iter()
            .enumerate()
            .filter(|(_, a)| a["before"] == at)
        {
            if action["kind"] == "stop" {
                let run = rig
                    .runtime
                    .block_on(
                        sqlx::query_scalar::<_, i64>(
                            "SELECT id FROM harness_runs WHERE execution_status = 'running'",
                        )
                        .fetch_one(&rig.pool),
                    )
                    .unwrap();
                rig.command(Command {
                    id: format!("stop-{index}"),
                    thread_id: Some(rig.thread),
                    kind: Kind::Stop { run_id: Some(run) },
                })
                .unwrap();
            } else {
                say(
                    &mut rig,
                    &format!("steer-{index}"),
                    action["text"].as_str().unwrap(),
                    Mode::Steer,
                );
            }
            rig.drain();
            assert_eq!(
                prompts(&rig),
                sent,
                "a correction cannot overlap the old prompt"
            );
        }
        observe(&mut rig, at);
        rig.feed(frame);
        rig.drain();
        if frame["method"] == "session/request_permission" {
            let request = rig
                .runtime
                .block_on(
                    sqlx::query_scalar("SELECT id FROM harness_requests WHERE status = 'pending'")
                        .fetch_one(&rig.pool),
                )
                .unwrap();
            rig.command(Command {
                id: format!("allow-{at}"),
                thread_id: Some(rig.thread),
                kind: Kind::Respond {
                    request_id: request,
                    option_id: "accept".into(),
                    feedback: None,
                    answers: None,
                },
            })
            .unwrap();
            rig.drain();
        }
        if frame.get("id").is_some_and(Value::is_number)
            && (frame.get("result").is_some() || frame.get("error").is_some())
        {
            outstanding -= 1;
        }
        let now = prompts(&rig);
        outstanding += now - sent;
        assert!(
            outstanding <= 1,
            "never two unanswered session/prompt calls"
        );
        sent = now;
    }
    rig.close();
    rig.drain();
    let held: Vec<bool> = rig
        .runtime
        .block_on(
            sqlx::query_scalar(
                "SELECT queue_held FROM harness_runs WHERE execution_status = 'queued'",
            )
            .fetch_all(&rig.pool),
        )
        .unwrap();
    if ["stop", "death", "error"]
        .iter()
        .any(|suffix| name == format!("antigravity-steer-{suffix}"))
    {
        assert_eq!(held, [true], "Stop and failure hold the correction: {name}");
    }
    rig.finish()
}

fn prompts(rig: &Rig) -> usize {
    rig.writes()
        .iter()
        .filter(|f| f["method"] == "session/prompt")
        .count()
}

fn events(replay: &Replay, kind: &str) -> usize {
    replay
        .envelopes
        .iter()
        .filter(|e| e["event"]["type"] == kind)
        .count()
}

fn one_restart(name: &str, steers: usize) -> Replay {
    let replay = recorded(name);
    let runs = replay.ledger["runs"].as_array().unwrap();
    let ran: Vec<_> = runs
        .iter()
        .filter(|r| r["execution_status"] != "steered")
        .collect();
    assert_eq!(ran.len(), 1);
    assert_eq!(ran[0]["execution_status"], "completed");
    let attempts = replay.ledger["attempts"].as_array().unwrap();
    assert_eq!(attempts.len(), 2);
    assert_eq!(attempts[0]["status"], "superseded");
    assert_eq!(attempts[1]["status"], "completed");
    assert!(attempts.iter().all(|a| a["run_id"] == ran[0]["id"]));
    let answer = replay
        .items
        .iter()
        .find(|i| i["id"] == ran[0]["answer_item_id"])
        .unwrap();
    assert_eq!(answer["content"], "AFTER_CANCEL_OK");
    assert_eq!(answer["attempt_id"], attempts[1]["id"]);
    assert_eq!(events(&replay, "run_settled"), 1);
    assert_eq!(events(&replay, "attempt_restarted"), 1);
    let rows: Vec<_> = replay
        .items
        .iter()
        .filter(|i| i["kind"] == "steer")
        .collect();
    assert_eq!(rows.len(), steers);
    for row in rows {
        assert_eq!(row["run_id"], ran[0]["id"]);
        let meta: Value = serde_json::from_str(row["meta"].as_str().unwrap()).unwrap();
        assert_eq!(meta["delivery"], "delivered");
    }
    assert!(replay
        .items
        .iter()
        .all(|i| i["kind"] != "unprompted" && i["kind"] != "interrupted"));
    assert!(replay.ledger["gaps"].as_array().unwrap().is_empty());
    replay
}

#[test]
fn a_restart_mid_text_keeps_partial_work_and_selects_only_the_second_attempts_answer() {
    let replay = one_restart("antigravity-steer-text", 1);
    let early = replay.items.iter().find(|i| i["content"] == "1.").unwrap();
    assert_eq!(early["display_role"], "commentary");
    assert_eq!(early["lifecycle"], "partial");
}

#[test]
fn a_restart_mid_command_keeps_the_failed_tool_and_reuses_the_native_session() {
    let replay = one_restart("antigravity-steer-tool", 1);
    let tool = replay.items.iter().find(|i| i["kind"] == "tool").unwrap();
    let meta: Value = serde_json::from_str(tool["meta"].as_str().unwrap()).unwrap();
    assert_eq!(meta["ok"], false);
    let prompts: Vec<_> = replay
        .outbound
        .iter()
        .filter(|f| f["method"] == "session/prompt")
        .collect();
    assert_eq!(
        prompts[0]["params"]["sessionId"],
        prompts[1]["params"]["sessionId"]
    );
}

#[test]
fn two_corrections_pending_on_one_cancel_restart_once_in_order() {
    let replay = one_restart("antigravity-steer-two", 2);
    assert_eq!(
        replay
            .outbound
            .iter()
            .filter(|f| f["method"] == "session/cancel")
            .count(),
        1
    );
    assert_eq!(
        replay.outbound.last().unwrap()["params"]["prompt"][0]["text"],
        "First correction.\n\nSecond correction."
    );
}

#[test]
fn a_cancel_that_loses_to_end_turn_starts_the_same_correction_as_its_own_run_once() {
    let replay = recorded("antigravity-steer-missed");
    let runs = replay.ledger["runs"].as_array().unwrap();
    assert_eq!(runs.len(), 2);
    assert!(runs.iter().all(|r| r["execution_status"] == "completed"));
    assert_eq!(events(&replay, "attempt_restarted"), 0);
    assert_eq!(events(&replay, "run_settled"), 2);
    assert!(replay.items.iter().all(|i| i["kind"] != "steer"));
    let correction = replay
        .items
        .iter()
        .find(|i| i["kind"] == "user" && i["content"] == "Reply AFTER_CANCEL_OK only. No tools.")
        .unwrap();
    assert_eq!(correction["run_id"], runs[1]["id"]);
    assert!(replay
        .envelopes
        .iter()
        .any(|e| e["event"]["type"] == "steer_updated"
            && e["event"]["message"] == runs[1]["id"]
            && e["event"]["delivery"] == "requeued"));
}

fn held_correction(name: &str, status: &str) {
    let replay = recorded(&format!("antigravity-steer-{name}"));
    let runs = replay.ledger["runs"].as_array().unwrap();
    assert_eq!(runs[0]["execution_status"], status, "{name}");
    assert_eq!(runs[1]["execution_status"], "queued", "{name}");
    assert_eq!(replay.ledger["attempts"].as_array().unwrap().len(), 1);
    assert_eq!(events(&replay, "attempt_restarted"), 0);
    assert_eq!(events(&replay, "run_settled"), 1);
    assert_eq!(
        replay
            .outbound
            .iter()
            .filter(|f| f["method"] == "session/prompt")
            .count(),
        1
    );
    assert!(replay.items.iter().all(|i| i["kind"] != "steer"));
}

#[test]
fn stop_after_a_restart_steer_holds_the_correction() {
    held_correction("stop", "interrupted");
}

#[test]
fn a_steer_during_stop_queues_as_a_new_message_and_waits_for_the_old_terminal() {
    let replay = recorded("antigravity-steer-during-stop");
    let runs = replay.ledger["runs"].as_array().unwrap();
    assert_eq!(runs.len(), 2);
    assert_eq!(runs[0]["execution_status"], "interrupted");
    assert_eq!(runs[1]["execution_status"], "completed");
    assert_eq!(events(&replay, "attempt_restarted"), 0);
    assert_eq!(events(&replay, "run_settled"), 2);
    assert!(replay.items.iter().all(|i| i["kind"] != "steer"));
}

#[test]
fn process_death_while_cancelling_fails_the_run_and_holds_the_correction() {
    held_correction("death", "failed");
}

#[test]
fn an_rpc_error_while_cancelling_fails_the_run_and_holds_the_correction() {
    held_correction("error", "failed");
}

#[test]
fn the_cancel_fixture_replays_without_relying_on_the_restart_capability() {
    use super::super::{fixture::Fixture, outbound::Outbound};
    use crate::harness::{event::unbound, HarnessEvent};
    use std::sync::{Arc, Mutex};
    let scratch = super::super::guard::scratch("agy-restart-wire");
    let (fixture, frames) = Fixture::load("antigravity-steer-text");
    let outbound = Outbound::new(fixture.expected_outbound, &scratch);
    let events: Arc<Mutex<Vec<HarnessEvent>>> = Arc::default();
    let heard = events.clone();
    let session = crate::harness::antigravity::acp::replay::session(
        outbound.clone(),
        Arc::new(move |e| heard.lock().unwrap().push(e)),
        &fixture.native_session,
        &scratch,
        false,
    );
    session.send("Replay request", &unbound()).unwrap();
    for frame in &frames[..3] {
        session.inbound(frame);
    }
    session.interrupt().unwrap();
    assert!(session.send("too early", &unbound()).is_err());
    assert_eq!(outbound.finish(false).len(), 2);
    session.inbound(&frames[3]);
    assert!(session.is_alive());
    session
        .send("Reply AFTER_CANCEL_OK only. No tools.", &unbound())
        .unwrap();
    for frame in &frames[4..] {
        session.inbound(frame);
    }
    outbound.finish(true);
    let terminals: Vec<_> = events
        .lock()
        .unwrap()
        .iter()
        .filter_map(|e| match e {
            HarnessEvent::TurnFinished { status, .. } => Some(status.clone()),
            _ => None,
        })
        .collect();
    assert_eq!(terminals, ["interrupted", "completed"]);
    session.kill();
}

#[test]
fn a_correction_to_the_restarted_prompt_waits_for_its_own_cancelled_reply() {
    let replay = recorded("antigravity-steer-successive");
    let attempts = replay.ledger["attempts"].as_array().unwrap();
    assert_eq!(attempts.len(), 3);
    assert_eq!(attempts[0]["status"], "superseded");
    assert_eq!(attempts[1]["status"], "superseded");
    assert_eq!(attempts[2]["status"], "completed");
    assert!(attempts
        .iter()
        .all(|a| a["run_id"] == attempts[0]["run_id"]));
    assert!(replay.items.iter().all(|i| i["kind"] != "interrupted"));
    assert_eq!(events(&replay, "attempt_restarted"), 2);
    assert_eq!(events(&replay, "run_settled"), 1);
}

#[test]
fn a_restart_keeps_the_waiting_message_behind_the_same_run_until_its_final_terminal() {
    use super::super::outbound::Expected;
    use serde_json::json;
    let replay = drive("antigravity-steer-text", |rig, at| {
        if at == 0 {
            let session = rig.scenario.fixture.native_session.clone();
            rig.outbound.expect([Expected {
                frame:json!({"jsonrpc":"2.0","id":7,"method":"session/prompt","params":{"sessionId":session,"prompt":[{"type":"text","text":"waiting"}]}}),
                response:Value::Null,error:None,any_prompt:false,
            }]);
            say(rig, "waiting", "waiting", Mode::Queue);
            rig.drain();
        }
        let queued: Vec<(String,bool)> = rig.runtime.block_on(sqlx::query_as(
            "SELECT queued_text, queue_held FROM harness_runs WHERE execution_status = 'queued'"
        ).fetch_all(&rig.pool)).unwrap();
        assert_eq!(queued, [("waiting".into(), false)]);
        assert!(rig
            .envelopes()
            .iter()
            .all(|e| e["event"]["type"] != "run_settled"));
        if at >= 4 {
            let attempts: Vec<(String, String)> = rig
                .runtime
                .block_on(
                    sqlx::query_as("SELECT reason, status FROM harness_attempts ORDER BY id")
                        .fetch_all(&rig.pool),
                )
                .unwrap();
            assert_eq!(
                attempts,
                [
                    ("initial".into(), "superseded".into()),
                    ("steering_restart".into(), "running".into())
                ]
            );
            assert_eq!(prompts(rig), 2);
            // A repeated old reply cannot finish or restart the new attempt.
            let stale = rig.scenario.frames[3].clone();
            rig.feed(&stale);
            rig.drain();
            assert_eq!(prompts(rig), 2);
        }
    });
    assert_eq!(
        replay.outbound.last().unwrap()["params"]["prompt"][0]["text"],
        "waiting"
    );
    assert_eq!(replay.ledger["runs"][0]["execution_status"], "completed");
}
