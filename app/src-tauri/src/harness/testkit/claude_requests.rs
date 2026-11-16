//! Claude's permission prompts replayed from the stage 2 probe recordings
//! (2.1.325): each answer is written to the prompt's own request id as the
//! manifest has it, each request is settled by what the recording did next,
//! and in Auto the classifier's and the deny rules' decisions land on the
//! call's row with nobody asked.

use serde_json::{json, Value};

use crate::harness::core::{Command, Kind};
use crate::harness::requests::{reasons, PermissionMode};

use super::replay::Replay;
use super::rig::Rig;
use super::scenario::Scenario;

/// What Oculus tells the agent when it declines unseen
/// (`providers/claude/approvals.rs`).
const UNSEEN: &str = "Oculus declined this without asking the student. Do not retry it or try another way to do the same thing.";

/// A recording's name, the answers its prompts get in the order they are
/// written (by the prompt's place in the recording), and whether the
/// student stops the turn once its first prompt is shown.
type Recorded = (&'static str, &'static [(usize, &'static str)], bool);

pub(super) const NAMES: [Recorded; 12] = [
    ("claude-approval-escape", &[(0, "accept")], false),
    ("claude-approval-escape-decline", &[(0, "decline")], false),
    ("claude-approval-write", &[(0, "accept")], false),
    ("claude-approval-write-decline", &[(0, "decline")], false),
    // Derived from the write-decline recording: no WebFetch prompt was recorded.
    ("claude-approval-tool-decline", &[(0, "decline")], false),
    (
        "claude-approval-network",
        &[(0, "accept"), (1, "accept")],
        false,
    ),
    // The recording's host answered the second prompt first.
    (
        "claude-approval-two",
        &[(1, "accept"), (0, "accept")],
        false,
    ),
    ("claude-approval-withdraw", &[], true),
    ("claude-approval-child", &[(0, "accept")], false),
    ("claude-approval-background", &[], false),
    ("claude-auto-classifier", &[], false),
    ("claude-auto-deny-rule", &[], false),
];

fn prompt(frame: &Value) -> bool {
    frame["type"] == "control_request" && frame["request"]["subtype"] == "can_use_tool"
}

/// The saved request for a prompt's request id, while it waits.
fn waiting(rig: &Rig, wire_id: &str) -> Option<i64> {
    rig.runtime
        .block_on(
            sqlx::query_scalar(
                "SELECT id FROM harness_requests WHERE native_ref = ?1 AND status = 'pending'",
            )
            .bind(json!(wire_id).to_string())
            .fetch_optional(&rig.pool),
        )
        .unwrap()
}

/// Replay `name` frame by frame. Each answer is given as soon as its
/// prompt is shown and every answer before it in the list has been given,
/// as the recording's host did; a stopped recording is stopped once its
/// prompt is shown.
pub(super) fn recorded(name: &str) -> Replay {
    let (_, answers, stop) = NAMES
        .iter()
        .find(|(n, ..)| *n == name)
        .unwrap_or_else(|| panic!("{name} is not a request recording"));
    let scenario = Scenario::recorded(name);
    let frames = scenario.frames.clone();
    let mut rig = Rig::start(scenario);
    rig.open();
    let prompts: Vec<String> = frames
        .iter()
        .filter(|f| prompt(f))
        .map(|f| f["request_id"].as_str().unwrap().to_string())
        .collect();
    let mut given = 0;
    let mut stopped = false;
    for frame in &frames {
        rig.feed(frame);
        rig.drain();
        while let Some((index, option)) = answers.get(given) {
            let Some(request) = waiting(&rig, &prompts[*index]) else {
                break;
            };
            respond(&mut rig, &format!("answer-{given}"), request, option).unwrap();
            given += 1;
        }
        if *stop && !stopped && prompt(frame) {
            stopped = true;
            stop_turn(&mut rig);
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

/// The tool rows' ref ids and meta, in order.
fn tools(rig: &Rig) -> Vec<(String, Value)> {
    let rows: Vec<(String, String)> = rig
        .runtime
        .block_on(
            sqlx::query_as(
                "SELECT ref_id, meta FROM harness_items WHERE kind = 'tool' ORDER BY id",
            )
            .fetch_all(&rig.pool),
        )
        .unwrap();
    rows.into_iter()
        .map(|(id, meta)| (id, serde_json::from_str(&meta).unwrap()))
        .collect()
}

fn tool(rig: &Rig, id: &str) -> Value {
    tools(rig)
        .into_iter()
        .find(|(ref_id, _)| ref_id == id)
        .unwrap_or_else(|| panic!("no tool row {id}"))
        .1
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

/// `name` fed through its first prompt, which is shown and waits.
fn after_prompt(scenario: Scenario) -> Rig {
    let mut scenario = scenario;
    let at = scenario.frames.iter().position(prompt).unwrap();
    scenario.gates.push(("after-prompt".into(), at + 1));
    let mut rig = Rig::start(scenario);
    rig.open();
    rig.until("after-prompt");
    rig
}

/// Feed `name` through its first prompt, answer it with `option`, then
/// feed the rest.
fn answered(name: &str, option: &str) -> Rig {
    let mut rig = after_prompt(Scenario::recorded(name));
    let pending = requests(&rig);
    assert_eq!(pending.len(), 1, "{name}");
    assert_eq!(pending[0].2, "pending");
    respond(&mut rig, "answer", pending[0].0, option).unwrap();
    rig.resume();
    rig.close();
    rig.drain();
    rig
}

#[test]
fn an_approved_escape_is_shown_as_outside_the_sandbox_and_runs() {
    let rig = answered("claude-approval-escape", "accept");
    let row = requests(&rig).remove(0);
    assert_eq!(row.1, "command");
    assert_eq!(
        (row.2.as_str(), row.3.as_deref(), row.4.as_str()),
        ("resolved", Some("accept"), "acknowledged"),
        "the pipe took the line: that is the proof"
    );
    assert!(row.6.is_some(), "it belongs to the run");
    let shown = payload(&rig, row.0);
    assert!(shown["command"].as_str().unwrap().starts_with("touch "));
    assert_eq!(shown["outsideSandbox"], true);
    assert_eq!(shown["tool"], "toolu_01NVgdeh9BxxhijwgLL36tud");
    assert_eq!(tool(&rig, "toolu_01NVgdeh9BxxhijwgLL36tud")["ok"], true);
    assert_eq!(run_status(&rig), "completed");
    rig.finish();
}

#[test]
fn a_declined_escape_does_not_run() {
    let rig = answered("claude-approval-escape-decline", "decline");
    let row = requests(&rig).remove(0);
    assert_eq!(
        (row.3.as_deref(), row.4.as_str()),
        (Some("decline"), "acknowledged")
    );
    let shown = payload(&rig, row.0);
    let call = shown["tool"].as_str().unwrap();
    assert_eq!(tool(&rig, call)["ok"], false);
    assert_eq!(run_status(&rig), "completed");
    rig.finish();
}

#[test]
fn a_write_outside_shows_its_file_and_is_made_once_approved() {
    let rig = answered("claude-approval-write", "accept");
    let row = requests(&rig).remove(0);
    assert_eq!(row.1, "file_change");
    let shown = payload(&rig, row.0);
    let change = &shown["changes"][0];
    assert!(change["path"]
        .as_str()
        .unwrap()
        .ends_with("/write-allow.txt"));
    assert_eq!(change["diff"], "+hello\n");
    assert!(
        shown.get("outsideSandbox").is_none(),
        "a file tool is not a command"
    );
    assert_eq!(tool(&rig, "toolu_01EDY2csR53PmdgUjJr5UL7S")["ok"], true);
    rig.finish();
}

#[test]
fn a_declined_write_is_not_made() {
    let rig = answered("claude-approval-write-decline", "decline");
    let row = requests(&rig).remove(0);
    assert_eq!(row.1, "file_change");
    assert_eq!(row.3.as_deref(), Some("decline"));
    let call = payload(&rig, row.0)["tool"].as_str().unwrap().to_string();
    assert_eq!(tool(&rig, &call)["ok"], false);
    rig.finish();
}

#[test]
fn a_web_fetch_is_asked_as_a_tool_call_and_a_declined_one_does_not_run() {
    let rig = answered("claude-approval-tool-decline", "decline");
    let row = requests(&rig).remove(0);
    assert_eq!(row.1, "tool");
    assert_eq!(
        (row.2.as_str(), row.3.as_deref(), row.4.as_str()),
        ("resolved", Some("decline"), "acknowledged")
    );
    assert!(row.6.is_some(), "it belongs to the run");
    assert_eq!(
        payload(&rig, row.0),
        json!({
            "name": "WebFetch",
            "kind": "fetch",
            "url": "https://example.com/",
            "prompt": "Return the page title.",
            "tool": "toolu_01UHEaTeaTaJzd5CDMxHcNVG",
        })
    );
    assert_eq!(tool(&rig, "toolu_01UHEaTeaTaJzd5CDMxHcNVG")["ok"], false);
    assert_eq!(run_status(&rig), "completed");
    rig.finish();
}

#[test]
fn a_new_host_is_asked_as_network_access_and_a_retry_outside_as_a_command() {
    let replay_rig = {
        let scenario = Scenario::recorded("claude-approval-network");
        let mut rig = Rig::start(scenario);
        rig.open();
        let frames = rig.scenario.frames.clone();
        let mut given = 0;
        for frame in &frames {
            rig.feed(frame);
            rig.drain();
            if prompt(frame) {
                let id = waiting(&rig, frame["request_id"].as_str().unwrap()).unwrap();
                respond(&mut rig, &format!("answer-{given}"), id, "accept").unwrap();
                given += 1;
            }
        }
        rig.close();
        rig.drain();
        rig
    };
    let rows = requests(&replay_rig);
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0].1, "network");
    assert_eq!(
        payload(&replay_rig, rows[0].0),
        json!({ "host": "example.com" }),
        "only the host: its call id is not the command's"
    );
    assert_eq!(rows[1].1, "command");
    assert_eq!(payload(&replay_rig, rows[1].0)["outsideSandbox"], true);
    assert!(rows.iter().all(|r| r.4 == "acknowledged"));
    let replay = replay_rig.finish();
    let network = &replay.outbound[1]["response"]["response"];
    assert_eq!(
        network,
        &json!({ "behavior": "allow", "updatedInput": { "host": "example.com" } }),
        "the suggested localSettings rule is never sent"
    );
}

#[test]
fn two_prompts_at_once_are_answered_out_of_order_on_their_own_ids() {
    let scenario = Scenario::recorded("claude-approval-two");
    let second_at = scenario
        .frames
        .iter()
        .enumerate()
        .filter(|(_, f)| prompt(f))
        .nth(1)
        .unwrap()
        .0;
    let mut scenario = scenario;
    scenario.gates.push(("both".into(), second_at + 1));
    let mut rig = Rig::start(scenario);
    rig.open();
    rig.until("both");
    let both = requests(&rig);
    assert_eq!(both.len(), 2);
    assert!(both.iter().all(|r| r.2 == "pending"));
    respond(&mut rig, "second", both[1].0, "accept").unwrap();
    assert_eq!(requests(&rig)[0].2, "pending", "its sibling still waits");
    respond(&mut rig, "first", both[0].0, "accept").unwrap();
    rig.resume();
    rig.close();
    rig.drain();
    let rows = requests(&rig);
    assert!(rows
        .iter()
        .all(|r| r.3.as_deref() == Some("accept") && r.4 == "acknowledged"));
    let replay = rig.finish();
    assert_eq!(
        replay.outbound[1]["response"]["request_id"],
        "a7b7ce7d-036c-4906-aaae-452382bc444b"
    );
}

#[test]
fn stop_with_a_prompt_pending_cancels_it_and_nothing_is_written_to_it() {
    let mut rig = after_prompt(Scenario::recorded("claude-approval-withdraw"));
    let request = requests(&rig)[0].0;
    stop_turn(&mut rig);
    rig.resume();
    rig.close();
    rig.drain();
    let row = requests(&rig).remove(0);
    assert_eq!(row.2, "cancelled");
    assert_eq!(row.5.as_deref(), Some(reasons::STOPPED));
    assert_eq!(row.4, "none", "the CLI withdrew it; no answer was sent");
    let late = respond(&mut rig, "late", request, "accept").unwrap_err();
    assert!(late.contains(reasons::STOPPED), "{late}");
    assert_eq!(run_status(&rig), "interrupted");
    let replay = rig.finish();
    assert_eq!(replay.outbound[1]["request"]["subtype"], "interrupt");
    assert_eq!(replay.outbound.len(), 2);
}

#[test]
fn a_subagents_prompt_is_labelled_with_its_agent_and_answered_on_its_own_id() {
    let rig = answered("claude-approval-child", "accept");
    let row = requests(&rig).remove(0);
    let shown = payload(&rig, row.0);
    assert_eq!(shown["agent"]["path"], "wire-probe");
    assert_eq!(shown["agent"]["tool"], "toolu_01DocbtPsUoApws1p4zHVjQz");
    assert_eq!(shown["agent"]["description"], "Wire probe foreground touch");
    assert_eq!(
        (row.3.as_deref(), row.4.as_str()),
        (Some("accept"), "acknowledged")
    );
    rig.finish();
}

#[test]
fn a_background_subagent_asking_after_its_turn_closed_is_declined_unseen() {
    let replay = recorded("claude-approval-background");
    assert!(
        !replay
            .envelopes
            .iter()
            .any(|e| e["event"]["type"] == "request_opened"),
        "nobody was asked"
    );
    assert_eq!(
        replay.outbound[1]["response"]["response"],
        json!({ "behavior": "deny", "message": UNSEEN })
    );
    assert!(replay
        .envelopes
        .iter()
        .any(|e| e["event"]["type"] == "unprompted_turn"));
}

#[test]
fn a_prompt_in_a_turn_claude_started_itself_has_no_run_and_ends_with_that_turn() {
    // Mutation, not a recording: the background subagent's prompt moved
    // into the turn the CLI started on its completion.
    let mut scenario = Scenario::recorded("claude-approval-background");
    let at = scenario.frames.iter().position(prompt).unwrap();
    let asked = scenario.frames.remove(at);
    let second_init = scenario
        .frames
        .iter()
        .enumerate()
        .filter(|(_, f)| f["type"] == "system" && f["subtype"] == "init")
        .nth(1)
        .unwrap()
        .0;
    let own_turn = second_init
        + scenario.frames[second_init..]
            .iter()
            .position(|f| f["type"] == "assistant")
            .unwrap();
    scenario.frames.insert(own_turn + 1, asked);
    scenario.gates.push(("asked".into(), own_turn + 2));
    let mut rig = Rig::start(scenario);
    rig.open();
    rig.until("asked");
    let row = requests(&rig).remove(0);
    assert_eq!(row.2, "pending", "shown inside the CLI's own turn");
    assert_eq!(row.6, None, "a turn of the CLI's own has no run");
    rig.resume();
    rig.close();
    rig.drain();
    let row = requests(&rig).remove(0);
    assert_eq!(row.2, "cancelled");
    assert_eq!(row.5.as_deref(), Some(reasons::TURN_ENDED));
    let replay = rig.finish();
    assert_eq!(
        replay.outbound[1]["response"]["response"]["message"], UNSEEN,
        "its CLI is told no once the turn ends"
    );
}

#[test]
fn in_auto_the_classifier_approves_and_nobody_is_asked() {
    let replay = recorded("claude-auto-classifier");
    assert!(!replay
        .envelopes
        .iter()
        .any(|e| e["event"]["type"] == "request_opened"));
    let reviewed: Vec<&Value> = replay
        .envelopes
        .iter()
        .filter(|e| e["event"]["type"] == "tool_reviewed")
        .map(|e| &e["event"]["review"])
        .collect();
    assert_eq!(reviewed.len(), 2, "the command and the Write outside");
    for review in reviewed {
        assert_eq!(review["status"], "approved");
        assert_eq!(review["source"], "classifier");
    }
    let rows: Vec<Value> = replay
        .items
        .iter()
        .filter(|i| i["kind"] == "tool")
        .map(|i| serde_json::from_str::<Value>(i["meta"].as_str().unwrap()).unwrap())
        .collect();
    assert!(rows
        .iter()
        .all(|m| m["review"]["source"] == "classifier" && m["ok"] == true));
}

#[test]
fn in_auto_a_deny_rule_refuses_a_protected_write_and_the_row_says_so() {
    let replay = recorded("claude-auto-deny-rule");
    let rows: Vec<Value> = replay
        .items
        .iter()
        .filter(|i| i["kind"] == "tool")
        .map(|i| serde_json::from_str::<Value>(i["meta"].as_str().unwrap()).unwrap())
        .collect();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0]["ok"], false);
    assert_eq!(rows[0]["review"]["status"], "denied");
    assert_eq!(rows[0]["review"]["source"], "rule");
}

/// The chat's mode saved mid-turn, as the composer's control saves it.
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

#[test]
fn dont_ask_saved_mid_turn_declines_the_next_prompt_unseen() {
    let mut scenario = Scenario::recorded("claude-approval-escape-decline").gate("asked", prompt);
    let deny = &mut scenario.fixture.expected_outbound[1].frame["response"]["response"];
    deny["message"] = json!(UNSEEN);
    let mut rig = Rig::start(scenario);
    rig.open();
    rig.until("asked");
    set_mode(&mut rig, PermissionMode::Quiet);
    rig.resume();
    rig.close();
    rig.drain();
    assert!(requests(&rig).is_empty(), "nobody was asked");
    assert_eq!(opened(&rig), 0);
    assert_eq!(run_status(&rig), "completed");
    rig.finish();
}

/// The write-decline recording with its prompt's file and `blocked_path`
/// set (relative to the library), and the folder the CLI suggests granting
/// set to the library itself, held once the prompt is fed. `unseen`: Oculus
/// declines it without asking.
fn write_into_library(file: &str, blocked: Option<&str>, unseen: bool) -> Rig {
    let mut scenario = Scenario::recorded("claude-approval-write-decline");
    let after = scenario.frames.iter().position(prompt).unwrap() + 1;
    scenario.gates.push(("asked".into(), after));
    if unseen {
        let deny = &mut scenario.fixture.expected_outbound[1].frame["response"]["response"];
        deny["message"] = json!(UNSEEN);
    }
    let mut rig = Rig::start(scenario);
    let library = rig.library();
    let at = rig.scenario.frames.iter().position(prompt).unwrap();
    let request = &mut rig.scenario.frames[at]["request"];
    request["input"]["file_path"] = json!(library.join(file).display().to_string());
    if let Some(blocked) = blocked {
        request["blocked_path"] = json!(library.join(blocked).display().to_string());
    }
    request["permission_suggestions"][0]["directories"] = json!([library.display().to_string()]);
    rig
}

#[test]
fn a_write_naming_a_protected_path_never_reaches_the_student() {
    // A file in a protected folder, a protected file at the library's root,
    // and a file whose `blocked_path` is in a protected folder.
    for (file, blocked) in [
        ("courses/notes.md", None),
        ("settings.json", None),
        ("agents/notes.md", Some("courses/notes.md")),
    ] {
        let mut rig = write_into_library(file, blocked, true);
        rig.open();
        rig.until("asked");
        rig.resume();
        rig.close();
        rig.drain();
        assert!(requests(&rig).is_empty(), "{file}");
        assert_eq!(opened(&rig), 0, "{file}");
        rig.finish();
    }
}

#[test]
fn an_unprotected_file_at_the_library_root_is_asked_though_claude_suggests_granting_the_library() {
    let mut rig = write_into_library("notes.md", None, false);
    rig.open();
    rig.until("asked");
    let row = requests(&rig).remove(0);
    assert_eq!((row.1.as_str(), row.2.as_str()), ("file_change", "pending"));
    let shown = payload(&rig, row.0);
    assert!(shown["changes"][0]["path"]
        .as_str()
        .unwrap()
        .ends_with("/notes.md"));
    respond(&mut rig, "answer", row.0, "decline").unwrap();
    rig.resume();
    rig.close();
    rig.drain();
    assert_eq!(requests(&rig)[0].3.as_deref(), Some("decline"));
    rig.finish();
}

#[test]
fn a_pipe_that_dies_mid_reply_leaves_the_answer_uncertain_and_never_resends_it() {
    let mut scenario = Scenario::recorded("claude-approval-escape");
    scenario.fixture.expected_outbound[1].error =
        Some("claude stdin: Broken pipe (os error 32)".into());
    let mut rig = after_prompt(scenario);
    let request = requests(&rig)[0].0;
    respond(&mut rig, "answer", request, "accept").unwrap();
    let row = requests(&rig).remove(0);
    assert_eq!((row.2.as_str(), row.4.as_str()), ("resolved", "uncertain"));
    let again = respond(&mut rig, "again", request, "decline").unwrap_err();
    assert!(again.contains("already answered"), "{again}");
    rig.resume();
    rig.close();
    rig.drain();
    let replay = rig.finish();
    assert_eq!(replay.outbound.len(), 2, "written once, never resent");
}

#[test]
fn a_prompt_pending_when_the_app_dies_expires_at_startup() {
    let rig = after_prompt(Scenario::recorded("claude-approval-escape"));
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
fn a_mode_change_ends_the_live_session_and_the_next_send_starts_another() {
    let mut scenario = Scenario::recorded("claude-auto-classifier");
    let again = scenario.fixture.expected_outbound[0].clone();
    scenario.fixture.expected_outbound.push(again);
    let mut rig = Rig::start(scenario);
    rig.open();
    rig.resume();
    rig.drain();
    assert!(!rig.busy());
    set_mode(&mut rig, PermissionMode::Ask);
    let replaced = rig.next_session();
    assert!(replaced.handle.is_alive());
    rig.send("second").unwrap();
    assert!(
        !replaced.handle.is_alive(),
        "Auto's settings keep commands sandboxed, so Ask needs a new process"
    );
    assert!(rig.alive(), "its successor took the message");
    rig.finish();
}

#[test]
fn every_claude_request_recording_replays_with_its_answers_written_exactly() {
    for (name, answers, stop) in NAMES {
        let replay = recorded(name);
        let written = replay.outbound.len() - 1;
        let unseen = usize::from(name == "claude-approval-background");
        assert_eq!(
            written,
            answers.len() + usize::from(stop) + unseen,
            "{name}"
        );
    }
}
