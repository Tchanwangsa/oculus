//! Mark's shaping of Ed's submission, against the probe's shape (SWEN20003
//! "Debugging", 207224), and Mark and the history against a fake keyd.

use std::time::Duration;

use serde_json::{json, Value};

use super::mark::submission::{normalise, Build};
use super::mark::{diff, history, standing};
use super::session::Session;
use super::session_tests::{context, go_online, slide};
use super::socket::Out;
use crate::sources::ed::workspace_state::{self, WorkspaceState};
use crate::sources::ed::Ed;
use crate::test_support::{FakeKeyd, Scratch};

fn pair(sign: &str, text: &str) -> (String, String) {
    (sign.to_string(), text.to_string())
}

/// A failed Java testcase as Ed marks it: the observed line said `Helo`
/// with no trailing newline where `Hello` was expected.
fn submission_json(id: i64) -> Value {
    json!({
        "id": id,
        "status": "failed",
        "workspace_hash": "4f2c",
        "created_at": "2026-10-13T03:12:44.120Z",
        "marked_at": "2026-10-13T03:12:44.693Z",
        "testcase_pass_count": 0,
        "testcase_total_count": 2,
        "feedback": "",
        "lesson_mark_id": null,
        "result": {
            "ok": true,
            "passed": false,
            "feedback": "",
            "build": {"ok": true, "output": "", "exit_status": {"type": "normal", "value": 0}},
            "testcases": [greeting_case(), hidden_case()],
        }
    })
}

fn greeting_case() -> Value {
    let diff = json!({
        "type": "check_diff",
        "passed": false,
        "value": {
            "expected": "Hello world\n",
            "observed": "Helo world",
            "diff": [
                [["-"], ["-", "Helo"], ["=", " world"], ["mn"]],
                [["+"], ["+", "Hello"], ["=", " world"]]
            ],
            "line_errors": 1,
            "char_errors": 1
        }
    });
    json!({
        "name": "Prints the greeting",
        "description": "",
        "hidden": false,
        "private": false,
        "min_score": 0,
        "max_score": 1,
        "score": 0,
        "skip": false,
        "ok": true,
        "passed": false,
        "feedback": "Check your spelling.",
        "run": {"ok": true, "output": "Helo world", "exit_status": {"type": "normal", "value": 0}},
        "checks": [{"type": "check_exit", "passed": true, "value": {"expected": 0}}, diff],
        "files": []
    })
}

fn hidden_case() -> Value {
    json!({
        "name": "Hidden case",
        "hidden": true,
        "private": true,
        "max_score": 1,
        "score": null,
        "ok": false,
        "run": null,
        "checks": []
    })
}

#[test]
fn eds_word_runs_become_sign_text_pairs_line_by_line() {
    let raw = json!([
        [["-"], ["-", "Helo"], ["=", " world"], ["mn"]],
        [["+"], ["+", "Hello"], ["=", " world"]],
        [["?"], ["~", "kept as is"]],
        "bare line",
        42,
    ]);
    assert_eq!(
        diff::lines(&raw),
        [
            vec![
                pair("-", ""),
                pair("-", "Helo"),
                pair("=", " world"),
                pair("mn", ""),
            ],
            vec![pair("+", ""), pair("+", "Hello"), pair("=", " world")],
            vec![pair("?", ""), pair("~", "kept as is")],
            vec![pair("=", "bare line")],
        ]
    );
    assert!(diff::lines(&Value::Null).is_empty());
}

#[test]
fn a_submission_is_shaped_for_the_feedback_panel() {
    let s = normalise(&submission_json(31)).unwrap();
    assert_eq!(s.id, 31);
    assert_eq!(s.status, "failed");
    assert_eq!(s.created_at, "2026-10-13T03:12:44.120Z");
    assert_eq!(s.marked_at.as_deref(), Some("2026-10-13T03:12:44.693Z"));
    assert_eq!(
        (s.testcase_pass_count, s.testcase_total_count),
        (Some(0), Some(2))
    );
    assert_eq!(s.feedback, None);
    let result = s.result.clone().unwrap();
    assert!(result.ok && !result.passed);
    assert_eq!(
        result.build,
        Some(Build {
            ok: true,
            output: String::new()
        })
    );
    let shown = &result.testcases[0];
    assert_eq!(shown.name, "Prints the greeting");
    assert_eq!(shown.description, None);
    assert_eq!((shown.score, shown.max_score), (Some(0.0), Some(1.0)));
    assert!(!shown.passed && !shown.skip && !shown.hidden);
    assert_eq!(shown.feedback.as_deref(), Some("Check your spelling."));
    assert_eq!(shown.output.as_deref(), Some("Helo world"));
    assert_eq!(shown.diffs.len(), 2, "only the check_diff check's lines");
    assert_eq!(shown.diffs[0][3], pair("mn", ""));
    let hidden = &result.testcases[1];
    assert!(hidden.hidden && hidden.private && !hidden.passed);
    assert_eq!((hidden.score, hidden.output.as_deref()), (None, None));
    assert!(hidden.diffs.is_empty());

    let wire = serde_json::to_value(&s).unwrap();
    assert_eq!(wire["testcasePassCount"], 0);
    assert_eq!(wire["markedAt"], "2026-10-13T03:12:44.693Z");
    assert_eq!(wire["result"]["testcases"][0]["maxScore"], 1.0);
    assert_eq!(
        wire["result"]["testcases"][0]["diffs"][1],
        json!([["+", ""], ["+", "Hello"], ["=", " world"]])
    );

    let bare = normalise(&json!({"id": 4})).unwrap();
    assert_eq!((bare.status.as_str(), bare.result), ("", None));
    assert!(normalise(&json!({"status": "x"})).is_err());
}

#[test]
fn standing_falls_back_to_the_slides_due_date() {
    let live =
        json!({"score": 0.5, "status": "attempted", "attempts_remaining": 2, "due_at": null});
    let s = standing::of(&live, Some("2026-10-20T13:00:00Z".into()));
    assert_eq!(s.score, Some(0.5));
    assert_eq!(s.status.as_deref(), Some("attempted"));
    assert_eq!(s.attempts_remaining, Some(2));
    assert_eq!(s.due_at.as_deref(), Some("2026-10-20T13:00:00Z"));
    let own = standing::of(&json!({"due_at": "2026-11-01T00:00:00Z"}), Some("x".into()));
    assert_eq!(own.due_at.as_deref(), Some("2026-11-01T00:00:00Z"));
    assert_eq!(own.attempts_remaining, None);
    let wire = serde_json::to_value(&own).unwrap();
    assert!(wire.get("attemptsRemaining").is_some() && wire.get("dueAt").is_some());
}

fn fake_ed(dir: &std::path::Path) -> FakeKeyd {
    FakeKeyd::start(dir, |header, _| {
        let body = match header["path"].as_str().unwrap() {
            "/api/challenges/267880/mark" => json!({"submission": submission_json(31)}),
            "/api/challenges/submissions/31" => json!({"submission": submission_json(31)}),
            "/api/challenges/submissions/30" => json!({"submission": submission_json(30)}),
            _ => return (json!({"status": 404, "headers": []}), b"{}".to_vec()),
        };
        (
            json!({"status": 200, "headers": []}),
            body.to_string().into_bytes(),
        )
    })
}

#[test]
fn mark_submits_a_fresh_commit_and_records_the_id() {
    let dir = Scratch::new("ed-ws-mark");
    let keyd = fake_ed(&dir);
    let (context, _) = context(&dir, "http://127.0.0.1:1");
    let (session, queue) = Session::detached(context.clone(), slide());

    let offline = session.mark(session.place()).unwrap_err();
    assert!(offline.contains("offline"), "{offline}");
    assert!(keyd.requests().is_empty(), "nothing submitted offline");

    let state = WorkspaceState {
        workspace_id: "W1".into(),
        base_commit: Some("h7".into()),
        submissions: vec![30],
        ..WorkspaceState::default()
    };
    workspace_state::save(&dir, 267880, &state).unwrap();
    go_online(&context);
    // Mark asks Ed for a commit rather than reusing the recorded base.
    let submission = std::thread::scope(|scope| {
        let marking = scope.spawn(|| session.mark(session.place()));
        let Ok(Out::Frame(frame)) = queue.recv_timeout(Duration::from_secs(5)) else {
            panic!("no commit frame")
        };
        let id = serde_json::from_str::<Value>(&frame).unwrap()["data"]["id"].clone();
        assert!(context
            .shared()
            .snapshots
            .answered(Some(&json!({ "id": id })), "h8:::s"));
        marking.join().unwrap()
    })
    .unwrap();
    assert_eq!(submission.id, 31);
    let requests = keyd.requests();
    assert_eq!(requests.len(), 1, "one mark, no retry");
    assert_eq!(requests[0].0["path"], "/api/challenges/267880/mark");
    assert_eq!(
        serde_json::from_slice::<Value>(&requests[0].1).unwrap(),
        json!({"hash": "h8:::s"})
    );
    assert_eq!(
        workspace_state::load(&dir, 267880).unwrap().submissions,
        [30, 31]
    );

    // The history reads newest first, and one Ed no longer has is left out.
    let mut state = workspace_state::load(&dir, 267880).unwrap();
    state.submissions.insert(0, 29);
    workspace_state::save(&dir, 267880, &state).unwrap();
    let ed = Ed::open(&dir);
    let read = history::submissions(&ed, &dir, 267880).unwrap();
    assert_eq!(read.iter().map(|s| s.id).collect::<Vec<_>>(), [31, 30]);
    assert!(history::submissions(&ed, &dir, 1).unwrap().is_empty());
}
