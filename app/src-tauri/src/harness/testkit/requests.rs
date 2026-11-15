//! Requests through the command path with a fake bridge (`Fake`): what a
//! CLI asks is saved with its route, answered once however often or from
//! however many panes it is clicked, delivered as the bridge proves it,
//! and settled by whatever ends its turn or its session.

use std::sync::{Arc, Mutex};

use serde_json::{json, Value};

use crate::harness::app::BusMsg;
use crate::harness::core::{decide::respond, Kind};
use crate::harness::requests::{
    reasons, Action, Decision, LiveRoute, NewRequest, PermissionMode, Reply, RequestOption, Route,
};
use crate::harness::{HarnessEvent, Provider, Release};

use super::commands::{ended, Bench};

/// A bridge's callback: it records what it was told, and its next write
/// proves what the test says.
#[derive(Default)]
pub(super) struct Fake {
    replies: Mutex<Vec<String>>,
    withdrawn: Mutex<usize>,
    next: Mutex<Option<Reply>>,
}

impl Route for Fake {
    fn reply(&self, option: &RequestOption) -> Reply {
        self.replies.lock().unwrap().push(option.id.clone());
        self.next
            .lock()
            .unwrap()
            .take()
            .unwrap_or(Reply::Acknowledged)
    }

    fn withdraw(&self) {
        *self.withdrawn.lock().unwrap() += 1;
    }
}

impl Fake {
    fn replies(&self) -> Vec<String> {
        self.replies.lock().unwrap().clone()
    }

    fn withdrawn(&self) -> usize {
        *self.withdrawn.lock().unwrap()
    }

    fn proves(&self, reply: Reply) {
        *self.next.lock().unwrap() = Some(reply);
    }
}

fn option(id: &str, decision: Decision) -> RequestOption {
    RequestOption {
        id: id.into(),
        label: id.into(),
        decision,
        scope: None,
    }
}

pub(super) fn command_request(native: Value, paths: &[&str]) -> NewRequest {
    NewRequest {
        action: Action::Command,
        payload: json!({ "command": "touch out.txt", "cwd": "/scratch" }),
        options: vec![
            option("accept", Decision::Accept),
            option("decline", Decision::Decline),
        ],
        native_ref: native.to_string(),
        paths: paths.iter().map(|p| p.to_string()).collect(),
    }
}

/// The CLI of session 1 asks; the test keeps the fake that will answer.
fn ask(bench: &Bench, native: Value, paths: &[&str]) -> (Arc<Fake>, BusMsg) {
    let fake = Arc::new(Fake::default());
    let asked = HarnessEvent::RequestAsked {
        request: command_request(native, paths),
        route: Some(LiveRoute(fake.clone())),
    };
    (fake, bench.session(asked))
}

/// A saved request: id, status, response, delivery, reason.
type Row = (i64, String, Option<String>, String, Option<String>);

/// A chat with a run open, its question sent under `id`.
fn turn_open(name: &str) -> Bench {
    let mut bench = Bench::new(name);
    let send = bench.send("send", "go").0;
    bench.play(vec![send]);
    assert!(bench.open());
    bench
}

fn requests(bench: &Bench) -> Vec<Row> {
    bench
        .runtime
        .block_on(
            sqlx::query_as(
                "SELECT id, status, response, delivery, reason FROM harness_requests ORDER BY id",
            )
            .fetch_all(&bench.pool),
        )
        .unwrap()
}

fn answer(bench: &mut Bench, id: &str, request: i64, option: &str) -> super::commands::Result {
    let kind = Kind::Respond {
        request_id: request,
        option_id: option.into(),
        feedback: None,
    };
    let (msg, got) = bench.command(id, kind);
    bench.play(vec![msg]);
    bench.got(got)
}

fn told(bench: &Bench, kind: &str) -> Vec<Value> {
    bench
        .events()
        .into_iter()
        .filter(|e| e["event"]["type"] == kind)
        .map(|e| e["event"]["request"].clone())
        .collect()
}

#[test]
fn a_request_is_saved_with_its_row_and_an_answer_goes_back_to_its_callback() {
    let mut bench = turn_open("request-approve");
    let (fake, asked) = ask(&bench, json!(0), &[]);
    bench.play(vec![asked]);
    let opened = told(&bench, "request_opened");
    assert_eq!(opened.len(), 1);
    let request = opened[0]["id"].as_i64().unwrap();
    assert_eq!(opened[0]["status"], "pending");
    assert_eq!(opened[0]["action"], "command");
    let row: (String, Option<String>, Option<i64>) = bench
        .runtime
        .block_on(
            sqlx::query_as("SELECT kind, ref_id, run_id FROM harness_items WHERE id = ?1")
                .bind(opened[0]["item_id"].as_i64().unwrap())
                .fetch_one(&bench.pool),
        )
        .unwrap();
    assert_eq!(row.0, "request");
    assert_eq!(
        row.1,
        Some(request.to_string()),
        "the row holds the id only"
    );
    assert!(row.2.is_some(), "the row is the open run's");
    // A request does not end or pause the run.
    assert!(bench.open());

    let outcome = answer(&mut bench, "answer", request, "accept").unwrap();
    assert_eq!(outcome.answered, Some(request));
    assert_eq!(fake.replies(), ["accept"]);
    assert_eq!(
        requests(&bench),
        [(
            request,
            "resolved".into(),
            Some("accept".into()),
            "acknowledged".into(),
            None
        )]
    );
    let updates: Vec<Value> = told(&bench, "request_updated")
        .into_iter()
        .map(|r| r["delivery"].clone())
        .collect();
    assert_eq!(updates, [json!("sending"), json!("acknowledged")]);
    assert!(bench.effects().contains(&"reply accept".to_string()));
}

#[test]
fn a_decline_is_written_as_the_callbacks_decline() {
    let mut bench = turn_open("request-decline");
    let (fake, asked) = ask(&bench, json!(4), &[]);
    bench.play(vec![asked]);
    let request = told(&bench, "request_opened")[0]["id"].as_i64().unwrap();
    answer(&mut bench, "no", request, "decline").unwrap();
    assert_eq!(fake.replies(), ["decline"]);
    assert_eq!(requests(&bench)[0].2.as_deref(), Some("decline"));
}

#[test]
fn two_requests_wait_side_by_side_and_are_answered_in_either_order() {
    let mut bench = turn_open("request-two");
    let (first, a) = ask(&bench, json!(0), &[]);
    let (second, b) = ask(&bench, json!(1), &[]);
    bench.play(vec![a, b]);
    let ids: Vec<i64> = told(&bench, "request_opened")
        .iter()
        .map(|r| r["id"].as_i64().unwrap())
        .collect();
    assert_eq!(ids.len(), 2);
    answer(&mut bench, "second", ids[1], "decline").unwrap();
    assert_eq!(requests(&bench)[0].1, "pending", "its sibling still waits");
    answer(&mut bench, "first", ids[0], "accept").unwrap();
    assert_eq!(first.replies(), ["accept"]);
    assert_eq!(second.replies(), ["decline"]);
}

#[test]
fn of_two_panes_clicking_one_answer_lands_and_the_same_id_again_writes_nothing() {
    let mut bench = turn_open("request-panes");
    let (fake, asked) = ask(&bench, json!(0), &[]);
    bench.play(vec![asked]);
    let request = told(&bench, "request_opened")[0]["id"].as_i64().unwrap();
    let first = answer(&mut bench, "pane-a", request, "accept").unwrap();
    let other = answer(&mut bench, "pane-b", request, "decline");
    assert_eq!(other.unwrap_err(), respond::ANSWERED);
    let again = answer(&mut bench, "pane-a", request, "accept").unwrap();
    assert_eq!(again, first, "the same receipt gives the same result");
    assert_eq!(fake.replies(), ["accept"], "one write to the callback");
}

#[test]
fn an_answer_the_request_does_not_offer_or_a_message_with_it_is_refused() {
    let mut bench = turn_open("request-options");
    let (fake, asked) = ask(&bench, json!(0), &[]);
    bench.play(vec![asked]);
    let request = told(&bench, "request_opened")[0]["id"].as_i64().unwrap();
    let wrong = answer(&mut bench, "wrong", request, "always");
    assert_eq!(wrong.unwrap_err(), respond::NO_SUCH_OPTION);
    let kind = Kind::Respond {
        request_id: request,
        option_id: "decline".into(),
        feedback: Some("do it differently".into()),
    };
    let (msg, got) = bench.command("feedback", kind);
    bench.play(vec![msg]);
    assert_eq!(bench.got(got).unwrap_err(), respond::NO_FEEDBACK);
    let elsewhere = answer(&mut bench, "missing", request + 10, "accept");
    assert_eq!(elsewhere.unwrap_err(), respond::NOT_HERE);
    assert!(fake.replies().is_empty());
    // A refusal records nothing: the same id works once it is lifted.
    answer(&mut bench, "wrong", request, "accept").unwrap();
}

#[test]
fn stop_while_a_request_waits_cancels_it_with_the_turn() {
    let mut bench = turn_open("request-stop");
    let (fake, asked) = ask(&bench, json!(0), &[]);
    bench.play(vec![asked]);
    let request = told(&bench, "request_opened")[0]["id"].as_i64().unwrap();
    let stop = bench.command("stop", Kind::Stop { run_id: None }).0;
    bench.play(vec![stop, bench.app(ended("interrupted"))]);
    let (_, status, _, _, reason) = requests(&bench).remove(0);
    assert_eq!(status, "cancelled");
    assert_eq!(reason.as_deref(), Some(reasons::STOPPED));
    let late = answer(&mut bench, "late", request, "accept").unwrap_err();
    assert!(late.contains(reasons::STOPPED), "{late}");
    assert!(fake.replies().is_empty());
    // The CLI retires its own callback after an interrupt, after the
    // terminal: that changes nothing now.
    bench.clear();
    let retired = HarnessEvent::RequestRetired {
        native_ref: "0".into(),
    };
    bench.play(vec![bench.session(retired)]);
    assert!(told(&bench, "request_updated").is_empty());
}

#[test]
fn a_process_that_dies_expires_its_requests_and_writes_nothing_to_them() {
    let mut bench = turn_open("request-exit");
    let (fake, asked) = ask(&bench, json!(0), &[]);
    bench.play(vec![asked]);
    let request = told(&bench, "request_opened")[0]["id"].as_i64().unwrap();
    bench.play(vec![BusMsg::Released {
        thread_id: bench.thread,
        provider: Provider::Claude,
        generation: 1,
        reason: Release::Exited,
    }]);
    let (_, status, _, _, reason) = requests(&bench).remove(0);
    assert_eq!(status, "expired");
    assert_eq!(reason.as_deref(), Some(reasons::PROCESS_ENDED));
    assert_eq!(fake.withdrawn(), 0, "a dead process is told nothing");
    let late = answer(&mut bench, "late", request, "accept").unwrap_err();
    assert!(late.contains(reasons::PROCESS_ENDED), "{late}");
    // The dead session's last words are not saved as a request.
    let (again, asked) = ask(&bench, json!(1), &[]);
    bench.play(vec![asked]);
    assert_eq!(requests(&bench).len(), 1);
    assert_eq!(again.withdrawn(), 1, "a request nobody will see is told no");
}

#[test]
fn a_replaced_session_cancels_its_requests_and_tells_its_cli_no() {
    let mut bench = turn_open("request-replaced");
    let (fake, asked) = ask(&bench, json!(0), &[]);
    bench.play(vec![asked]);
    bench.play(vec![BusMsg::Released {
        thread_id: bench.thread,
        provider: Provider::Claude,
        generation: 1,
        reason: Release::Replaced,
    }]);
    let (_, status, _, _, reason) = requests(&bench).remove(0);
    assert_eq!(status, "cancelled");
    assert_eq!(reason.as_deref(), Some(reasons::REPLACED));
    assert_eq!(fake.withdrawn(), 1);
}

#[test]
fn a_request_found_pending_at_startup_expires_and_cannot_be_answered() {
    let mut bench = turn_open("request-startup");
    let (_, asked) = ask(&bench, json!(0), &[]);
    let (_, answered) = ask(&bench, json!(1), &[]);
    bench.play(vec![asked, answered]);
    let ids: Vec<i64> = told(&bench, "request_opened")
        .iter()
        .map(|r| r["id"].as_i64().unwrap())
        .collect();
    // The provider had the run's prompt: startup interrupts it rather than
    // putting it back in its queue.
    bench
        .runtime
        .block_on(
            sqlx::query("UPDATE harness_runs SET execution_status = 'running'")
                .execute(&bench.pool),
        )
        .unwrap();
    // The second's answer was written and never confirmed.
    bench
        .runtime
        .block_on(
            sqlx::query(
                "UPDATE harness_requests SET status = 'resolved', response = 'accept',
                        delivery = 'sending' WHERE id = ?1",
            )
            .bind(ids[1])
            .execute(&bench.pool),
        )
        .unwrap();
    // The production startup path, on the same database file.
    let startup = crate::harness::app::startup::Startup::default();
    let pool = bench.pool.clone();
    bench
        .runtime
        .block_on(startup.ready(|| async move { Ok(pool) }))
        .unwrap();
    let rows = requests(&bench);
    assert_eq!(rows[0].1, "expired");
    assert_eq!(rows[0].4.as_deref(), Some(reasons::APP_CLOSED));
    assert_eq!(rows[1].1, "resolved");
    assert_eq!(rows[1].3, "uncertain", "it may have arrived; never resent");
    let late = answer(&mut bench, "late", ids[0], "accept").unwrap_err();
    assert!(late.contains(reasons::APP_CLOSED), "{late}");
}

#[test]
fn a_pipe_that_dies_mid_reply_leaves_the_answer_uncertain_and_never_resends_it() {
    let mut bench = turn_open("request-uncertain");
    let (fake, asked) = ask(&bench, json!(0), &[]);
    bench.play(vec![asked]);
    let request = told(&bench, "request_opened")[0]["id"].as_i64().unwrap();
    fake.proves(Reply::Uncertain("broken pipe".into()));
    answer(&mut bench, "once", request, "accept").unwrap();
    assert_eq!(requests(&bench)[0].3, "uncertain");
    let again = answer(&mut bench, "twice", request, "accept");
    assert_eq!(again.unwrap_err(), respond::ANSWERED);
    assert_eq!(fake.replies(), ["accept"]);
}

#[test]
fn a_write_that_reached_nothing_can_be_answered_again() {
    let mut bench = turn_open("request-failed");
    let (fake, asked) = ask(&bench, json!(0), &[]);
    bench.play(vec![asked]);
    let request = told(&bench, "request_opened")[0]["id"].as_i64().unwrap();
    fake.proves(Reply::Failed("no process".into()));
    answer(&mut bench, "once", request, "accept").unwrap();
    let row = requests(&bench).remove(0);
    assert_eq!((row.1.as_str(), row.3.as_str()), ("pending", "failed"));
    answer(&mut bench, "twice", request, "decline").unwrap();
    assert_eq!(fake.replies(), ["accept", "decline"]);
    assert_eq!(requests(&bench)[0].3, "acknowledged");
}

#[test]
fn a_written_answer_is_acknowledged_when_the_cli_retires_its_callback_by_its_own_id() {
    let mut bench = turn_open("request-native-ids");
    // Codex sent numbers; its schema allows strings. Each is kept as sent.
    let (number, a) = ask(&bench, json!(7), &[]);
    let (string, b) = ask(&bench, json!("7"), &[]);
    bench.play(vec![a, b]);
    let ids: Vec<i64> = told(&bench, "request_opened")
        .iter()
        .map(|r| r["id"].as_i64().unwrap())
        .collect();
    let natives: Vec<String> = bench
        .runtime
        .block_on(
            sqlx::query_scalar("SELECT native_ref FROM harness_requests ORDER BY id")
                .fetch_all(&bench.pool),
        )
        .unwrap();
    assert_eq!(natives, ["7", "\"7\""]);
    number.proves(Reply::Written);
    answer(&mut bench, "number", ids[0], "accept").unwrap();
    assert_eq!(requests(&bench)[0].3, "sending");
    // The string's callback is withdrawn; the number's is confirmed.
    let retire = |native: &str| HarnessEvent::RequestRetired {
        native_ref: native.into(),
    };
    bench.play(vec![
        bench.session(retire("\"7\"")),
        bench.session(retire("7")),
    ]);
    let rows = requests(&bench);
    assert_eq!(rows[0].3, "acknowledged");
    assert_eq!(rows[1].1, "expired");
    assert_eq!(rows[1].4.as_deref(), Some(reasons::WITHDRAWN));
    assert!(string.replies().is_empty());
}

#[test]
fn a_request_before_its_tool_row_is_shown_and_the_tool_row_follows() {
    let mut bench = turn_open("request-before-tool");
    let (_, asked) = ask(&bench, json!(0), &[]);
    let tool = HarnessEvent::ToolStarted {
        id: "call-1".into(),
        kind: crate::harness::ToolKind::Bash,
        name: "commandExecution".into(),
        title: "touch out.txt".into(),
        input: json!({}),
    };
    bench.play(vec![asked, bench.session(tool)]);
    let kinds: Vec<String> = bench
        .runtime
        .block_on(
            sqlx::query_scalar("SELECT kind FROM harness_items ORDER BY id").fetch_all(&bench.pool),
        )
        .unwrap();
    assert_eq!(kinds, ["user", "request", "tool"]);
}

#[test]
fn a_request_naming_a_protected_path_never_reaches_the_student() {
    let mut bench = turn_open("request-protected");
    let library = bench.library();
    let protected = library.join("courses/maths/notes.md");
    let (fake, asked) = ask(&bench, json!(0), &[protected.to_str().unwrap()]);
    let outside = library.join("agents/scratch.md");
    let (allowed, fine) = ask(&bench, json!(1), &[outside.to_str().unwrap()]);
    bench.play(vec![asked, fine]);
    assert_eq!(fake.withdrawn(), 1, "the CLI is told no");
    assert_eq!(allowed.withdrawn(), 0);
    assert_eq!(told(&bench, "request_opened").len(), 1);
    assert_eq!(requests(&bench).len(), 1);
}

#[test]
fn a_request_with_no_turn_open_is_declined_unseen() {
    let mut bench = Bench::new("request-no-turn");
    let (fake, asked) = ask(&bench, json!(0), &[]);
    bench.play(vec![asked]);
    assert_eq!(fake.withdrawn(), 1);
    assert!(requests(&bench).is_empty());
    assert!(told(&bench, "request_opened").is_empty());
}

#[test]
fn deleting_the_chat_tells_its_waiting_callbacks_no() {
    let mut bench = turn_open("request-delete");
    let (fake, asked) = ask(&bench, json!(0), &[]);
    bench.play(vec![asked]);
    let delete = bench.command("delete", Kind::DeleteThread).0;
    bench.play(vec![delete]);
    assert_eq!(fake.withdrawn(), 1);
    assert!(requests(&bench).is_empty(), "the rows went with the chat");
}

#[test]
fn the_mode_and_the_provider_stay_while_a_request_waits() {
    let mut bench = turn_open("request-mode");
    bench.allow_handoff();
    let set = |bench: &mut Bench, id: &str, mode| {
        let (msg, got) = bench.command(id, Kind::SetPermissionMode { mode });
        bench.play(vec![msg]);
        bench.got(got)
    };
    set(&mut bench, "quiet", PermissionMode::Quiet).unwrap();
    assert_eq!(
        bench.count("SELECT COUNT(*) FROM harness_threads WHERE permission_mode = 'quiet'"),
        1
    );
    assert!(bench
        .events()
        .iter()
        .any(|e| e["event"]["type"] == "permission_mode" && e["event"]["mode"] == "quiet"));
    let (_, asked) = ask(&bench, json!(0), &[]);
    bench.play(vec![asked]);
    let refused = set(&mut bench, "auto", PermissionMode::Auto).unwrap_err();
    assert_eq!(refused, respond::MODE_WHILE_PENDING);
    let kind = Kind::SelectModel {
        provider: Provider::Codex,
        model: Some("gpt".into()),
        effort: None,
    };
    let (msg, got) = bench.command("switch", kind);
    bench.play(vec![msg]);
    assert_eq!(
        bench.got(got).unwrap_err(),
        "Claude Code is waiting for you to answer a request in this chat. Answer it before switching."
    );
    // Its own model can still be picked.
    let kind = Kind::SelectModel {
        provider: Provider::Claude,
        model: Some("sonnet".into()),
        effort: None,
    };
    let (msg, got) = bench.command("model", kind);
    bench.play(vec![msg]);
    bench.got(got).unwrap();
}

#[test]
fn a_reload_reads_each_request_with_its_state() {
    let mut bench = turn_open("request-load");
    let (_, asked) = ask(&bench, json!(0), &[]);
    bench.play(vec![asked]);
    let request = told(&bench, "request_opened")[0]["id"].as_i64().unwrap();
    answer(&mut bench, "answer", request, "decline").unwrap();
    let snapshot = bench
        .runtime
        .block_on(crate::harness::core::load::read(&bench.pool, bench.thread))
        .unwrap();
    let loaded = serde_json::to_value(&snapshot.requests).unwrap();
    assert_eq!(loaded[0]["id"], request);
    assert_eq!(loaded[0]["status"], "resolved");
    assert_eq!(loaded[0]["response"], "decline");
    assert_eq!(loaded[0]["options"][0]["id"], "accept");
    let last = told(&bench, "request_updated").pop().unwrap();
    assert_eq!(loaded[0], last, "a reload reads what the last event told");
}
