//! Codex approvals replayed from the stage 2 probe recordings: each answer
//! is written to the callback id the CLI sent (the manifest holds the exact
//! frame), each request is settled by what the recording did next, and in
//! Auto the reviewer's decision lands on the command's row with nobody
//! asked.

use serde_json::{json, Value};

use crate::harness::core::{Command, Kind};
use crate::harness::requests::reasons;

use super::replay::Replay;
use super::rig::Rig;
use super::scenario::Scenario;

/// The recordings this file replays, each with the answer its callbacks got
/// (by callback id) and whether the student stopped the turn before its
/// end. The registry (`matrix.rs`) replays them through [`recorded`].
/// A recording's name, its answers by callback id, and whether it was stopped.
type Recorded = (&'static str, &'static [(&'static str, &'static str)], bool);

pub(super) const NAMES: [Recorded; 12] = [
    ("codex-approval-command", &[("0", "accept")], false),
    ("codex-approval-decline", &[("0", "decline")], false),
    ("codex-approval-file", &[("0", "accept")], false),
    ("codex-approval-permissions", &[("0", "accept")], false),
    (
        "codex-approval-two",
        &[("1", "decline"), ("0", "accept")],
        false,
    ),
    ("codex-approval-withdraw", &[], true),
    ("codex-approval-child", &[("0", "decline")], false),
    ("codex-session-grant", &[("0", "accept_for_session")], false),
    ("codex-session-grant-reuse", &[], false),
    ("codex-mode-switch", &[("0", "decline")], false),
    ("codex-auto-review", &[], false),
    ("codex-auto-review-denied", &[], false),
];

/// Replay `name` frame by frame, answering each callback just before the
/// frame that retires it, as the recording's client did.
pub(super) fn recorded(name: &str) -> Replay {
    let (_, answers, stop) = NAMES
        .iter()
        .find(|(n, ..)| *n == name)
        .unwrap_or_else(|| panic!("{name} is not a request recording"));
    let scenario = Scenario::recorded(name);
    let frames = scenario.frames.clone();
    let mut rig = Rig::start(scenario);
    rig.open();
    let mut stopped = false;
    for frame in &frames {
        rig.drain();
        let native = frame["params"]["requestId"].to_string();
        let answer = answers.iter().find(|(id, _)| *id == native);
        if let (true, Some((_, option))) = (resolved(frame), answer) {
            let request: i64 = rig
                .runtime
                .block_on(
                    sqlx::query_scalar("SELECT id FROM harness_requests WHERE native_ref = ?1")
                        .bind(&native)
                        .fetch_one(&rig.pool),
                )
                .unwrap();
            respond(&mut rig, &format!("answer-{native}"), request, option).unwrap();
        }
        if *stop && !stopped && method(frame, "turn/completed") {
            stopped = true;
            let thread = rig.thread;
            rig.command(Command {
                id: "stop".into(),
                thread_id: Some(thread),
                kind: Kind::Stop { run_id: None },
            })
            .unwrap();
        }
        rig.feed(frame);
    }
    rig.drain();
    rig.close();
    rig.drain();
    rig.finish()
}

fn method(frame: &Value, name: &str) -> bool {
    frame["method"] == name
}

fn resolved(frame: &Value) -> bool {
    method(frame, "serverRequest/resolved")
}

/// A saved request: id, action, status, response, delivery, reason.
type Row = (i64, String, String, Option<String>, String, Option<String>);

/// The chat's requests.
fn requests(rig: &Rig) -> Vec<Row> {
    rig.runtime
        .block_on(
            sqlx::query_as(
                "SELECT id, action, status, response, delivery, reason
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
        },
    });
    rig.drain();
    outcome.map(|_| ())
}

/// The tool rows' meta, in order.
fn tools(rig: &Rig) -> Vec<Value> {
    let metas: Vec<String> = rig
        .runtime
        .block_on(
            sqlx::query_scalar("SELECT meta FROM harness_items WHERE kind = 'tool' ORDER BY id")
                .fetch_all(&rig.pool),
        )
        .unwrap();
    metas
        .iter()
        .map(|m| serde_json::from_str(m).unwrap())
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

/// Feed `name` up to its first callback's retirement, answer the one
/// request with `option`, then feed the rest.
fn answered(name: &str, option: &str) -> Rig {
    let scenario = Scenario::recorded(name).gate("resolved", resolved);
    let mut rig = Rig::start(scenario);
    rig.open();
    rig.until("resolved");
    let pending = requests(&rig);
    assert_eq!(pending.len(), 1, "{name}");
    assert_eq!(pending[0].2, "pending");
    respond(&mut rig, "answer", pending[0].0, option).unwrap();
    assert_eq!(requests(&rig)[0].4, "sending", "written, not yet retired");
    rig.resume();
    rig.close();
    rig.drain();
    rig
}

#[test]
fn an_approved_command_is_answered_on_its_callback_and_runs() {
    let rig = answered("codex-approval-command", "accept");
    let row = requests(&rig).remove(0);
    assert_eq!(row.1, "command");
    assert_eq!(
        (row.2.as_str(), row.3.as_deref(), row.4.as_str()),
        ("resolved", Some("accept"), "acknowledged")
    );
    let shown = payload(&rig, row.0);
    assert!(shown["command"].as_str().unwrap().contains("touch"));
    assert_eq!(shown["outsideSandbox"], true);
    assert!(shown["tool"].as_str().unwrap().starts_with("exec-"));
    assert_eq!(tools(&rig)[0]["ok"], true);
    assert_eq!(run_status(&rig), "completed");
    rig.finish();
}

#[test]
fn a_declined_command_does_not_run() {
    let rig = answered("codex-approval-decline", "decline");
    let row = requests(&rig).remove(0);
    assert_eq!(
        (row.3.as_deref(), row.4.as_str()),
        (Some("decline"), "acknowledged")
    );
    assert_eq!(tools(&rig)[0]["ok"], false);
    assert_eq!(run_status(&rig), "completed");
    rig.finish();
}

#[test]
fn a_file_change_request_shows_the_paths_and_diff_of_its_item() {
    let rig = answered("codex-approval-file", "accept");
    let row = requests(&rig).remove(0);
    assert_eq!(row.1, "file_change");
    let shown = payload(&rig, row.0);
    let change = &shown["changes"][0];
    assert!(change["path"].as_str().unwrap().ends_with("dummy.txt"));
    assert!(change["diff"].as_str().unwrap().contains("+accepted dummy"));
    assert_eq!(row.4, "acknowledged");
    rig.finish();
}

#[test]
fn a_permissions_request_is_granted_its_own_profile_for_the_turn() {
    let rig = answered("codex-approval-permissions", "accept");
    let row = requests(&rig).remove(0);
    assert_eq!(row.1, "permissions");
    let shown = payload(&rig, row.0);
    assert!(shown["permissions"]["fileSystem"]["write"][0].is_string());
    assert!(
        shown.get("outsideSandbox").is_none(),
        "a grant is not an escalation"
    );
    rig.finish();
}

#[test]
fn an_exact_command_grant_is_offered_with_its_scope_and_sent_as_accept_for_session() {
    let rig = answered("codex-session-grant", "accept_for_session");
    let row = requests(&rig).remove(0);
    assert_eq!(row.3.as_deref(), Some("accept_for_session"));
    let options: String = rig
        .runtime
        .block_on(
            sqlx::query_scalar("SELECT options FROM harness_requests WHERE id = ?1")
                .bind(row.0)
                .fetch_one(&rig.pool),
        )
        .unwrap();
    let options: Value = serde_json::from_str(&options).unwrap();
    let grant = &options[1];
    assert_eq!(grant["decision"], "accept_for_session");
    let scope = grant["scope"].as_str().unwrap();
    assert!(scope.contains("this exact command") && scope.contains("this chat"));
    assert!(!scope.contains("restart"), "not proven to survive one");
    rig.finish();
}

#[test]
fn after_an_exact_command_grant_the_next_turn_runs_it_without_asking() {
    let mut rig = Rig::start(Scenario::recorded("codex-session-grant-reuse"));
    rig.open();
    rig.resume();
    rig.close();
    rig.drain();
    assert!(requests(&rig).is_empty());
    assert_eq!(tools(&rig)[0]["ok"], true);
    rig.finish();
}

#[test]
fn two_requests_at_once_are_answered_out_of_order_on_their_own_ids() {
    let scenario = Scenario::recorded("codex-approval-two")
        .gate("first", |f| resolved(f) && f["params"]["requestId"] == 1)
        .gate("second", |f| resolved(f) && f["params"]["requestId"] == 0);
    let mut rig = Rig::start(scenario);
    rig.open();
    rig.until("first");
    let both = requests(&rig);
    assert_eq!(both.len(), 2);
    // The manifest says the second callback (id 1) is answered first.
    respond(&mut rig, "decline-second", both[1].0, "decline").unwrap();
    rig.until("second");
    assert_eq!(requests(&rig)[0].2, "pending", "its sibling still waits");
    respond(&mut rig, "accept-first", both[0].0, "accept").unwrap();
    rig.resume();
    rig.close();
    rig.drain();
    let rows = requests(&rig);
    assert_eq!(rows[0].3.as_deref(), Some("accept"));
    assert_eq!(rows[1].3.as_deref(), Some("decline"));
    assert!(rows.iter().all(|r| r.4 == "acknowledged"));
    rig.finish();
}

#[test]
fn stop_with_a_request_pending_cancels_it_and_its_late_retirement_changes_nothing() {
    let scenario = Scenario::recorded("codex-approval-withdraw")
        .gate("terminal", |f| method(f, "turn/completed"));
    let mut rig = Rig::start(scenario);
    rig.open();
    rig.until("terminal");
    let request = requests(&rig)[0].0;
    let thread = rig.thread;
    // Stop asks Codex to interrupt (the manifest's second write); no answer
    // is written to the callback.
    rig.command(Command {
        id: "stop".into(),
        thread_id: Some(thread),
        kind: Kind::Stop { run_id: None },
    })
    .unwrap();
    rig.resume();
    rig.close();
    rig.drain();
    let row = requests(&rig).remove(0);
    assert_eq!(row.2, "cancelled");
    assert_eq!(row.5.as_deref(), Some(reasons::STOPPED));
    assert_eq!(row.4, "none");
    let late = respond(&mut rig, "late", request, "accept").unwrap_err();
    assert!(late.contains(reasons::STOPPED), "{late}");
    assert_eq!(run_status(&rig), "interrupted");
    rig.finish();
}

#[test]
fn a_childs_request_is_labelled_with_its_agent_and_answered_on_the_childs_callback() {
    let rig = answered("codex-approval-child", "decline");
    let row = requests(&rig).remove(0);
    let shown = payload(&rig, row.0);
    assert_eq!(shown["agent"]["path"], "/root/child_approval_probe");
    assert_ne!(shown["agent"]["thread"], json!(""));
    assert_eq!(
        (row.3.as_deref(), row.4.as_str()),
        (Some("decline"), "acknowledged")
    );
    rig.finish();
}

#[test]
fn a_request_pending_when_the_app_dies_expires_at_startup() {
    let scenario = Scenario::recorded("codex-approval-command").gate("resolved", resolved);
    let mut rig = Rig::start(scenario);
    rig.open();
    rig.until("resolved");
    let request = requests(&rig)[0].0;
    let rig = rig.restart();
    rig.startup().unwrap();
    let row = requests(&rig).remove(0);
    assert_eq!(row.2, "expired");
    assert_eq!(row.5.as_deref(), Some(reasons::APP_CLOSED));
    let mut rig = rig;
    let late = respond(&mut rig, "after-restart", request, "accept").unwrap_err();
    assert!(late.contains(reasons::APP_CLOSED), "{late}");
}

#[test]
fn in_auto_the_reviewers_approval_lands_on_the_command_and_nobody_is_asked() {
    let mut rig = Rig::start(Scenario::recorded("codex-auto-review"));
    rig.open();
    rig.resume();
    rig.close();
    rig.drain();
    assert!(requests(&rig).is_empty());
    let tool = tools(&rig).remove(0);
    assert_eq!(tool["ok"], true);
    assert_eq!(tool["review"]["status"], "approved");
    assert_eq!(tool["review"]["risk"], "low");
    assert!(tool["review"]["rationale"].as_str().unwrap().len() > 10);
    assert!(rig
        .envelopes()
        .iter()
        .any(|e| e["event"]["type"] == "tool_reviewed"));
    rig.finish();
}

#[test]
fn in_auto_a_reviewers_denial_declines_the_command_and_the_turn_goes_on() {
    let mut rig = Rig::start(Scenario::recorded("codex-auto-review-denied"));
    rig.open();
    rig.resume();
    rig.close();
    rig.drain();
    assert!(
        requests(&rig).is_empty(),
        "a denial is final; nobody is asked"
    );
    let denied = tools(&rig)
        .into_iter()
        .find(|t| t["review"]["status"] == "denied")
        .expect("the denied command's row");
    assert_eq!(denied["ok"], false);
    assert_eq!(denied["review"]["risk"], "medium");
    assert_eq!(run_status(&rig), "completed");
    rig.finish();
}

#[test]
fn a_thread_started_under_auto_review_asks_the_student_once_its_turn_names_the_user() {
    let rig = answered("codex-mode-switch", "decline");
    assert_eq!(requests(&rig)[0].3.as_deref(), Some("decline"));
    assert_eq!(tools(&rig)[0]["ok"], false);
    rig.finish();
}

#[test]
fn every_request_recording_replays_with_its_answers_written_exactly() {
    for (name, answers, _) in NAMES {
        let replay = recorded(name);
        let written = replay.outbound.len() - 1;
        let interrupt = usize::from(name == "codex-approval-withdraw");
        assert_eq!(written, answers.len() + interrupt, "{name}");
    }
}
