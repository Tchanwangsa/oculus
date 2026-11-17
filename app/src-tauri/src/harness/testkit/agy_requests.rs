//! Antigravity over ACP, replayed from the stage 5 probe recordings (agy
//! 1.5.9): Rust's policy answers first, the student answers the rest, each
//! answer goes to the request's own id as the manifest has it, and Stop is
//! `session/cancel`.

use std::sync::{Arc, Mutex};

use serde_json::{json, Value};

use crate::harness::core::{Command, Kind};
use crate::harness::requests::{reasons, PermissionMode};
use crate::harness::HarnessEvent;

use super::outbound::{Expected, Outbound};
use super::replay::Replay;
use super::rig::Rig;
use super::scenario::Scenario;

/// A recording's name, the answers its requests get in the order they are
/// written (by the request's place among those put to the student), and
/// the frame the student stops the turn before, if they do.
type Recorded = (
    &'static str,
    &'static [(usize, &'static str)],
    Option<fn(&Value) -> bool>,
);

fn failed_tool(frame: &Value) -> bool {
    frame["params"]["update"]["status"] == "failed"
}

fn reply(frame: &Value) -> bool {
    frame.get("result").is_some() && frame.get("method").is_none()
}

pub(super) const NAMES: [Recorded; 5] = [
    // The write is in the chat's folder: Rust allows it, so only pwd and
    // false are asked.
    (
        "antigravity-acp-full-turn",
        &[(0, "accept"), (1, "accept")],
        None,
    ),
    ("antigravity-acp-write-decline", &[(0, "decline")], None),
    ("antigravity-acp-command-decline", &[(0, "decline")], None),
    (
        "antigravity-acp-cancel",
        &[(0, "accept")],
        Some(failed_tool),
    ),
    ("antigravity-acp-cancel-text", &[], Some(reply)),
];

fn permission(frame: &Value) -> bool {
    frame["method"] == "session/request_permission"
}

/// The saved request for a permission request's id, while it waits.
fn waiting(rig: &Rig, id: &Value) -> Option<i64> {
    rig.runtime
        .block_on(
            sqlx::query_scalar(
                "SELECT id FROM harness_requests WHERE native_ref = ?1 AND status = 'pending'",
            )
            .bind(id.to_string())
            .fetch_optional(&rig.pool),
        )
        .unwrap()
}

/// Replay `name` frame by frame, answering each request put to the student
/// as soon as it is shown, and stopping where the recording's client did.
pub(super) fn recorded(name: &str) -> Replay {
    let (_, answers, stop) = NAMES
        .iter()
        .find(|(n, ..)| *n == name)
        .unwrap_or_else(|| panic!("{name} is not an ACP recording"));
    let scenario = Scenario::recorded(name);
    let frames = scenario.frames.clone();
    let mut rig = Rig::start(scenario);
    rig.open();
    let mut asked: Vec<Value> = Vec::new();
    let mut given = 0;
    let mut stopped = false;
    for frame in &frames {
        if let Some(stop) = stop {
            if !stopped && stop(frame) {
                stopped = true;
                stop_turn(&mut rig);
            }
        }
        rig.feed(frame);
        rig.drain();
        if permission(frame) && waiting(&rig, &frame["id"]).is_some() {
            asked.push(frame["id"].clone());
        }
        while let Some((index, option)) = answers.get(given) {
            let Some(request) = asked.get(*index).and_then(|id| waiting(&rig, id)) else {
                break;
            };
            respond(&mut rig, &format!("answer-{given}"), request, option).unwrap();
            given += 1;
        }
    }
    assert_eq!(given, answers.len(), "{name}: every answer was given");
    rig.close();
    rig.drain();
    rig.finish()
}

fn stop_turn(rig: &mut Rig) {
    let thread = rig.thread;
    rig.command(Command {
        id: "stop".into(),
        thread_id: Some(thread),
        kind: Kind::Stop { run_id: None },
    })
    .unwrap();
    rig.drain();
}

fn set_mode(rig: &mut Rig, mode: PermissionMode) {
    let thread = rig.thread;
    rig.command(Command {
        id: format!("mode-{}", mode.as_str()),
        thread_id: Some(thread),
        kind: Kind::SetPermissionMode { mode },
    })
    .unwrap();
    rig.drain();
}

/// A saved request: id, action, status, response, delivery, reason, run.
type Row = (
    i64,
    String,
    String,
    Option<String>,
    String,
    Option<String>,
    Option<i64>,
);

fn requests(rig: &Rig) -> Vec<Row> {
    rig.runtime
        .block_on(
            sqlx::query_as(
                "SELECT id, action, status, response, delivery, reason, run_id
                   FROM harness_requests ORDER BY id",
            )
            .fetch_all(&rig.pool),
        )
        .unwrap()
}

fn payload(rig: &Rig, id: i64) -> Value {
    let raw: String = rig
        .runtime
        .block_on(
            sqlx::query_scalar("SELECT payload FROM harness_requests WHERE id = ?1")
                .bind(id)
                .fetch_one(&rig.pool),
        )
        .unwrap();
    serde_json::from_str(&raw).unwrap()
}

fn respond(rig: &mut Rig, id: &str, request: i64, option: &str) -> Result<(), String> {
    let thread = rig.thread;
    let outcome = rig.command(Command {
        id: id.into(),
        thread_id: Some(thread),
        kind: Kind::Respond {
            request_id: request,
            option_id: option.into(),
            feedback: None,
            answers: None,
        },
    });
    rig.drain();
    outcome.map(|_| ())
}

/// The rows of `kind`: ref id, content and meta, in order.
fn rows(rig: &Rig, kind: &str) -> Vec<(String, String, Value)> {
    let rows: Vec<(Option<String>, Option<String>, Option<String>)> = rig
        .runtime
        .block_on(
            sqlx::query_as(
                "SELECT ref_id, content, meta FROM harness_items WHERE kind = ?1 ORDER BY id",
            )
            .bind(kind)
            .fetch_all(&rig.pool),
        )
        .unwrap();
    rows.into_iter()
        .map(|(id, content, meta)| {
            let meta = meta
                .and_then(|m| serde_json::from_str(&m).ok())
                .unwrap_or(Value::Null);
            (id.unwrap_or_default(), content.unwrap_or_default(), meta)
        })
        .collect()
}

fn run_status(rig: &Rig) -> String {
    rig.runtime
        .block_on(
            sqlx::query_scalar("SELECT execution_status FROM harness_runs ORDER BY id DESC")
                .fetch_one(&rig.pool),
        )
        .unwrap()
}

fn opened(rig: &Rig) -> usize {
    rig.envelopes()
        .iter()
        .filter(|e| e["event"]["type"] == "request_opened")
        .count()
}

/// `scenario` fed up to the `n`th permission request put to the student,
/// which is shown and waits.
fn at_request(scenario: Scenario, n: usize) -> Rig {
    let mut scenario = scenario;
    let at = scenario
        .frames
        .iter()
        .enumerate()
        .filter(|(_, f)| permission(f))
        .nth(n)
        .unwrap()
        .0;
    scenario.gates.push(("asked".into(), at + 1));
    let mut rig = Rig::start(scenario);
    rig.open();
    rig.until("asked");
    rig
}

fn finished(mut rig: Rig) -> Rig {
    rig.resume_drained();
    rig.close();
    rig.drain();
    rig
}

#[test]
fn a_full_turn_writes_its_folder_unasked_and_asks_for_each_command() {
    let mut rig = at_request(Scenario::recorded("antigravity-acp-full-turn"), 1);
    let pwd = requests(&rig);
    assert_eq!(pwd.len(), 1, "the write in the chat's folder was not asked");
    assert_eq!(pwd[0].1, "command");
    let shown = payload(&rig, pwd[0].0);
    assert_eq!(shown["command"], "pwd");
    assert_eq!(shown["reason"], "Print working directory");
    assert_eq!(shown["outsideSandbox"], false);
    assert_eq!(shown["tool"], "call_13071268180472458926");
    respond(&mut rig, "pwd", pwd[0].0, "accept").unwrap();
    rig.until_next(permission);
    let second = requests(&rig)[1].0;
    respond(&mut rig, "false", second, "accept").unwrap();
    let rig = finished(rig);
    for row in requests(&rig) {
        assert_eq!(
            (row.2.as_str(), row.3.as_deref(), row.4.as_str()),
            ("resolved", Some("accept"), "acknowledged"),
            "the pipe took the line: that is the proof"
        );
        assert!(row.6.is_some(), "it belongs to the run");
    }
    let tools = rows(&rig, "tool");
    let ok: Vec<(&str, Value)> = tools
        .iter()
        .map(|(id, _, meta)| (id.as_str(), meta["ok"].clone()))
        .collect();
    assert_eq!(
        ok,
        [
            ("call_1137422119217318604", json!(true)),
            ("call_13071268180472458926", json!(true)),
            ("call_2879428469444077703", json!(false)),
            ("call_13744566986877724097", json!(false)),
        ],
        "the write, pwd, false (exit 1), the missing file"
    );
    let answers = rows(&rig, "assistant");
    assert_eq!(answers.len(), 1);
    assert_eq!(answers[0].1, "FULL_DONE");
    assert_eq!(run_status(&rig), "completed");
    let replay = rig.finish();
    assert_eq!(replay.outbound.len(), 4);
    assert_eq!(
        replay.outbound[1],
        json!({ "jsonrpc": "2.0", "id": "1",
                "result": { "outcome": { "outcome": "selected", "optionId": "allow" } } }),
        "Rust allowed the write itself"
    );
}

#[test]
fn a_write_outside_is_asked_as_a_file_change_and_a_declined_one_is_not_made() {
    let mut rig = at_request(Scenario::recorded("antigravity-acp-write-decline"), 0);
    let row = requests(&rig).remove(0);
    assert_eq!(row.1, "file_change");
    let shown = payload(&rig, row.0);
    assert_eq!(
        shown["changes"][0]["path"],
        "/tmp/oculus-agy-outside-XXXXXX/native.txt"
    );

    assert_eq!(shown["changes"][0]["diff"], "+outside-file-probe\n");
    respond(&mut rig, "no", row.0, "decline").unwrap();
    let rig = finished(rig);
    let row = requests(&rig).remove(0);
    assert_eq!(
        (row.2.as_str(), row.3.as_deref()),
        ("resolved", Some("decline"))
    );
    let tools = rows(&rig, "tool");
    assert_eq!(tools[0].2["ok"], false, "the declined write failed");
    assert!(
        !rows(&rig, "thinking").is_empty(),
        "high effort's thought is kept"
    );
    assert_eq!(run_status(&rig), "completed");
    rig.finish();
}

#[test]
fn a_declined_command_does_not_run() {
    let mut rig = at_request(Scenario::recorded("antigravity-acp-command-decline"), 0);
    let row = requests(&rig).remove(0);
    assert_eq!(
        payload(&rig, row.0)["command"],
        "touch restart-allow-check.txt"
    );
    respond(&mut rig, "no", row.0, "decline").unwrap();
    let rig = finished(rig);
    assert_eq!(rows(&rig, "tool")[0].2["ok"], false);
    assert_eq!(run_status(&rig), "completed");
    let replay = rig.finish();
    assert_eq!(replay.outbound[1]["result"]["outcome"]["optionId"], "deny");
}

/// The write-decline recording with its write aimed at `file` in the
/// library, every place the request names it.
fn write_into_library(file: &str) -> Rig {
    let scenario = Scenario::recorded("antigravity-acp-write-decline");
    let mut rig = Rig::start(scenario);
    let path = format!("$LIBRARY/{file}");
    let outside = "/private/tmp/oculus-agy-outside-XXXXXX/native.txt";
    let outside_short = "/tmp/oculus-agy-outside-XXXXXX/native.txt";
    for frame in &mut rig.scenario.frames {
        let text = frame
            .to_string()
            .replace(outside, &path)
            .replace(outside_short, &path);
        *frame = serde_json::from_str(&text).unwrap();
    }
    rig
}

#[test]
fn a_write_naming_a_protected_path_or_the_database_never_reaches_the_student() {
    for file in [
        "courses/maths/native.txt",
        "courses/maths/Course/Recordings/x/native.txt",
        "settings.json",
        "vault.bin",
        "agents/skills/x/SKILL.md",
        "oculus.db",
        "oculus.db-wal",
    ] {
        let mut rig = write_into_library(file);
        rig.open();
        let rig = finished(rig);
        assert!(requests(&rig).is_empty(), "{file}: nobody was asked");
        assert_eq!(opened(&rig), 0, "{file}");
        assert_eq!(run_status(&rig), "completed", "{file}");
        let replay = rig.finish();
        assert_eq!(
            replay.outbound[1]["result"]["outcome"]["optionId"], "deny",
            "{file}: Rust refused it"
        );
    }
}

#[test]
fn dont_ask_declines_a_request_unseen_and_auto_asks_as_ask_does() {
    let mut scenario = Scenario::recorded("antigravity-acp-command-decline");
    scenario.fixture.permission_mode = Some(PermissionMode::Quiet);
    let rig = finished({
        let mut rig = Rig::start(scenario);
        rig.open();
        rig
    });
    assert!(requests(&rig).is_empty(), "nobody was asked");
    assert_eq!(run_status(&rig), "completed");
    rig.finish();

    let mut scenario = Scenario::recorded("antigravity-acp-command-decline");
    scenario.fixture.permission_mode = Some(PermissionMode::Auto);
    let mut rig = at_request(scenario, 0);
    let asked = requests(&rig);
    assert_eq!(asked.len(), 1, "no reviewer: Auto asks");
    respond(&mut rig, "no", asked[0].0, "decline").unwrap();
    finished(rig).finish();
}

#[test]
fn dont_ask_saved_mid_turn_declines_the_next_request_unseen() {
    let mut scenario =
        Scenario::recorded("antigravity-acp-command-decline").gate("asked", permission);
    scenario.fixture.permission_mode = Some(PermissionMode::Ask);
    let mut rig = Rig::start(scenario);
    rig.open();
    rig.until("asked");
    set_mode(&mut rig, PermissionMode::Quiet);
    let rig = finished(rig);
    assert!(requests(&rig).is_empty());
    rig.finish();
}

#[test]
fn stop_while_a_command_runs_cancels_the_prompt_and_the_process_lives_on() {
    let replay = recorded("antigravity-acp-cancel");
    assert_eq!(
        replay.outbound[2],
        json!({ "jsonrpc": "2.0", "method": "session/cancel",
                "params": { "sessionId": "c1e3f868-73fa-4696-945e-861027e169b4" } })
    );
    let runs = replay.ledger["runs"].as_array().unwrap();
    assert_eq!(runs.last().unwrap()["execution_status"], "interrupted");
    let tool = replay
        .items
        .iter()
        .find(|i| i["kind"] == "tool")
        .expect("the command's row");
    let meta: Value = serde_json::from_str(tool["meta"].as_str().unwrap()).unwrap();
    assert_eq!(meta["ok"], false, "the cancelled command failed");
}

#[test]
fn stop_while_the_answer_streams_keeps_the_text_so_far() {
    let replay = recorded("antigravity-acp-cancel-text");
    let answer = replay
        .items
        .iter()
        .find(|i| i["kind"] == "assistant")
        .expect("the text so far");
    assert_eq!(answer["content"], "1.");
    let runs = replay.ledger["runs"].as_array().unwrap();
    assert_eq!(runs.last().unwrap()["execution_status"], "interrupted");
}

#[test]
fn stop_with_a_request_waiting_answers_it_cancelled_and_the_turn_ends_stopped() {
    // Mutation: the recording is cut after pwd's request and given the
    // reply a cancelled prompt gets (`07-cancel`).
    let mut scenario = Scenario::recorded("antigravity-acp-full-turn");
    let at = scenario
        .frames
        .iter()
        .enumerate()
        .filter(|(_, f)| permission(f))
        .nth(1)
        .unwrap()
        .0;
    scenario.frames.truncate(at + 1);
    scenario
        .frames
        .push(json!({ "jsonrpc": "2.0", "id": 4, "result": { "stopReason": "cancelled" } }));
    let session = scenario.fixture.native_session.clone();
    scenario.fixture.expected_outbound.truncate(2);
    scenario.fixture.expected_outbound.extend([
        Expected {
            frame: json!({ "jsonrpc": "2.0", "method": "session/cancel",
                           "params": { "sessionId": session } }),
            response: Value::Null,
            error: None,
            any_prompt: false,
        },
        Expected {
            frame: json!({ "jsonrpc": "2.0", "id": "2",
                           "result": { "outcome": { "outcome": "cancelled" } } }),
            response: Value::Null,
            error: None,
            any_prompt: false,
        },
    ]);
    let mut rig = at_request(scenario, 1);
    let request = requests(&rig)[0].0;
    stop_turn(&mut rig);
    // Before the turn's end an answer is taken, but the stop already
    // answered the request: nothing is written, and it can be answered again.
    respond(&mut rig, "late-before-end", request, "accept").unwrap();
    let row = requests(&rig).remove(0);
    assert_eq!((row.2.as_str(), row.4.as_str()), ("pending", "failed"));
    let mut rig = finished(rig);
    let row = requests(&rig).remove(0);
    assert_eq!(row.2, "cancelled");
    assert_eq!(row.5.as_deref(), Some(reasons::STOPPED));
    let late = respond(&mut rig, "late", request, "accept").unwrap_err();
    assert!(late.contains(reasons::STOPPED), "{late}");
    assert_eq!(run_status(&rig), "interrupted");
    let replay = rig.finish();
    assert_eq!(replay.outbound.len(), 4);
}

#[test]
fn a_process_that_dies_mid_turn_fails_the_run_and_keeps_its_text_as_partial() {
    // Mutation: the recording ends before the prompt's reply.
    let mut scenario = Scenario::recorded("antigravity-acp-full-turn");
    scenario.frames.retain(|f| !reply(f));
    let replay = recorded_scenario(scenario, &[(0, "accept"), (1, "accept")]);
    let runs = replay.ledger["runs"].as_array().unwrap();
    assert_eq!(runs.last().unwrap()["execution_status"], "failed");
    let answer = replay
        .items
        .iter()
        .find(|i| i["kind"] == "assistant")
        .expect("the cut-off text");
    assert_eq!(answer["content"], "FULL_DONE");
    assert_eq!(
        replay.ledger["gaps"].as_array().map(Vec::len),
        Some(1),
        "a gap says the answer may be incomplete: {}",
        replay.ledger
    );
}

/// [`recorded`] for a scenario a test changed.
fn recorded_scenario(scenario: Scenario, answers: &[(usize, &str)]) -> Replay {
    let frames = scenario.frames.clone();
    let mut rig = Rig::start(scenario);
    rig.open();
    let mut asked: Vec<Value> = Vec::new();
    let mut given = 0;
    for frame in &frames {
        rig.feed(frame);
        rig.drain();
        if permission(frame) && waiting(&rig, &frame["id"]).is_some() {
            asked.push(frame["id"].clone());
        }
        while let Some((index, option)) = answers.get(given) {
            let Some(request) = asked.get(*index).and_then(|id| waiting(&rig, id)) else {
                break;
            };
            respond(&mut rig, &format!("answer-{given}"), request, option).unwrap();
            given += 1;
        }
    }
    rig.close();
    rig.drain();
    rig.finish()
}

#[test]
fn a_request_pending_when_the_app_dies_expires_at_startup() {
    let rig = at_request(Scenario::recorded("antigravity-acp-full-turn"), 1);
    let request = requests(&rig)[0].0;
    let mut rig = rig.restart();
    rig.startup().unwrap();
    let row = requests(&rig).remove(0);
    assert_eq!(row.2, "expired");
    assert_eq!(row.5.as_deref(), Some(reasons::APP_CLOSED));
    let late = respond(&mut rig, "after-restart", request, "accept").unwrap_err();
    assert!(late.contains(reasons::APP_CLOSED), "{late}");
}

#[test]
fn a_pipe_that_dies_mid_reply_leaves_the_answer_uncertain_and_never_resends_it() {
    let mut scenario = Scenario::recorded("antigravity-acp-command-decline");
    scenario.fixture.expected_outbound[1].error =
        Some("agy stdin: Broken pipe (os error 32)".into());
    let mut rig = at_request(scenario, 0);
    let request = requests(&rig)[0].0;
    respond(&mut rig, "answer", request, "decline").unwrap();
    let row = requests(&rig).remove(0);
    assert_eq!((row.2.as_str(), row.4.as_str()), ("resolved", "uncertain"));
    let again = respond(&mut rig, "again", request, "accept").unwrap_err();
    assert!(again.contains("already answered"), "{again}");
    let rig = finished(rig);
    let replay = rig.finish();
    assert_eq!(replay.outbound.len(), 2, "written once, never resent");
}

#[test]
fn a_one_off_turn_answers_by_policy_alone_and_finishes_without_asking() {
    let scratch = super::guard::scratch("agy-one-off");
    let root = super::guard::assert_scratch(&scratch, &scratch);
    let scenario = Scenario::recorded("antigravity-acp-full-turn");
    let answer = |id: &str, option: &str| Expected {
        frame: json!({ "jsonrpc": "2.0", "id": id,
                       "result": { "outcome": { "outcome": "selected", "optionId": option } } }),
        response: Value::Null,
        error: None,
        any_prompt: false,
    };
    let mut expected = vec![scenario.fixture.expected_outbound[0].clone()];
    // The write in its folder is allowed; nobody is asked about pwd or false.
    expected.extend([
        answer("1", "allow"),
        answer("2", "deny"),
        answer("3", "deny"),
    ]);
    let outbound = Outbound::new(expected, &root);
    let events: Arc<Mutex<Vec<HarnessEvent>>> = Arc::default();
    let heard = events.clone();
    let sink: crate::harness::Sink = Arc::new(move |e| heard.lock().unwrap().push(e));
    let session = crate::harness::antigravity::acp::replay::session(
        outbound.clone(),
        sink,
        &scenario.fixture.native_session,
        &root,
        true,
    );
    session
        .send("Replay request", &crate::harness::event::unbound())
        .unwrap();
    for frame in &scenario.frames {
        session.inbound(&crate::harness::antigravity::acp::replay::place(
            frame, &root,
        ));
    }
    outbound.finish(true);
    let events = events.lock().unwrap();
    assert!(
        !events
            .iter()
            .any(|e| matches!(e, HarnessEvent::RequestAsked { .. })),
        "a one-off turn asks nobody"
    );
    assert!(matches!(
        events.last(),
        Some(HarnessEvent::TurnFinished { status, .. }) if status == "completed"
    ));
    drop(events);
    let _ = std::fs::remove_dir_all(&scratch);
}

#[test]
fn a_one_off_bypassed_oculus_command_is_allowed_without_a_question() {
    let scratch = super::guard::scratch("agy-oculus-bypass");
    let scenario = Scenario::recorded("antigravity-acp-full-turn");
    let mut params = scenario
        .frames
        .iter()
        .find(|f| permission(f) && f["params"]["toolCall"]["name"] == "run_command")
        .unwrap()["params"]
        .clone();
    params["toolCall"]["rawInput"]["CommandLine"] = json!("oculus memory add fact");
    params["toolCall"]["rawInput"]["BypassSandbox"] = json!(true);
    let outbound = Outbound::new(
        vec![
            scenario.fixture.expected_outbound[0].clone(),
            Expected {
                frame: json!({"jsonrpc":"2.0","id":"bypass","result":{"outcome":{"outcome":"selected","optionId":"allow"}}}),
                response: Value::Null,
                error: None,
                any_prompt: false,
            },
        ],
        &scratch,
    );
    let events: Arc<Mutex<Vec<HarnessEvent>>> = Arc::default();
    let heard = events.clone();
    let session = crate::harness::antigravity::acp::replay::session(
        outbound.clone(),
        Arc::new(move |e| heard.lock().unwrap().push(e)),
        &scenario.fixture.native_session,
        &scratch,
        true,
    );
    session
        .send("Replay request", &crate::harness::event::unbound())
        .unwrap();
    session.inbound(&json!({"jsonrpc":"2.0","id":"bypass","method":"session/request_permission","params":params}));
    outbound.finish(true);
    assert!(!events
        .lock()
        .unwrap()
        .iter()
        .any(|e| matches!(e, HarnessEvent::RequestAsked { .. })));
    session.kill();
    drop(session);
    let _ = std::fs::remove_dir_all(scratch);
}

#[test]
fn every_acp_recording_replays_with_its_answers_written_exactly() {
    for (name, answers, stop) in NAMES {
        let replay = recorded(name);
        // The prompt, Rust's own answers, the student's, and a stop's cancel.
        let rust = usize::from(name == "antigravity-acp-full-turn");
        assert_eq!(
            replay.outbound.len(),
            1 + rust + answers.len() + usize::from(stop.is_some()),
            "{name}"
        );
        assert!(!replay.busy, "{name}");
    }
}

#[test]
fn a_stop_agy_never_answers_reaches_force_stop_which_kills_only_that_chats_process() {
    // Mutation: the cancel recording with the cancelled reply withheld, so
    // the stop goes unanswered.
    let mut scenario = Scenario::recorded("antigravity-acp-cancel");
    scenario.frames.retain(|f| !reply(f) && !failed_tool(f));
    let mut rig = at_request(scenario, 0);
    let request = requests(&rig)[0].0;
    respond(&mut rig, "yes", request, "accept").unwrap();
    rig.resume_drained();
    let run: i64 = rig
        .runtime
        .block_on(sqlx::query_scalar("SELECT id FROM harness_runs").fetch_one(&rig.pool))
        .unwrap();
    let thread = rig.thread;
    rig.command(Command {
        id: "stop".into(),
        thread_id: Some(thread),
        kind: Kind::Stop { run_id: Some(run) },
    })
    .unwrap();
    // The rig's clock jumps to the consumer's next wake when the bus is idle.
    rig.drain();
    let told = rig
        .envelopes()
        .into_iter()
        .find(|e| e["event"]["type"] == "stop_unresponsive")
        .expect("agy said nothing for fifteen seconds");
    assert_eq!(told["event"]["others"], 0, "a process of its own");
    assert!(rig.alive());
    rig.command(Command {
        id: "force".into(),
        thread_id: Some(thread),
        kind: Kind::ForceStop { run_id: run },
    })
    .unwrap();
    rig.drain();
    assert!(!rig.alive(), "its process was killed");
    assert_eq!(run_status(&rig), "interrupted");
    rig.close();
    rig.drain();
    rig.finish();
}
