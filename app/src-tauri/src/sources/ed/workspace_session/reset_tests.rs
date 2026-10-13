//! Reset to scaffold against a fake keyd (Ed's `/api`) and a fake workspace
//! server: Ed's tree replaces the folder and the record, terminals end.

use std::path::Path;
use std::sync::mpsc::Receiver;

use serde_json::{json, Value};

use super::context::sha256;
use super::events::{Body, TermState};
use super::session::Session;
use super::session_tests::{context, go_online, slide, zip_of, SLIDE_DIR};
use super::socket::Out;
use super::terminal::{self, PtySize};
use crate::sources::ed::workspace_api::user_id_in;
use crate::sources::ed::workspace_state::{self, WorkspaceState};
use crate::test_support::{FakeKeyd, FakeServer, Reply, Scratch};

const RESET: &str = "/api/users/42/challenges/267880/reset";

/// Ed's `/api`: the reset answers with an empty body, as a 204 would.
fn fake_ed(dir: &Path) -> FakeKeyd {
    FakeKeyd::start(dir, |header, _| {
        let body = match header["path"].as_str().unwrap_or("") {
            RESET => Vec::new(),
            "/api/user" => json!({"user": {"id": 42, "name": "Ada"}})
                .to_string()
                .into_bytes(),
            "/api/workspaces/W1/download" => json!({"ticket": "T-DOWN"}).to_string().into_bytes(),
            _ => json!({}).to_string().into_bytes(),
        };
        (json!({"status": 200, "headers": []}), body)
    })
}

fn scaffold() -> FakeServer {
    let zip = zip_of(&[("tally.py", b"count = 0\n"), ("src/new.txt", b"n\n")]);
    FakeServer::start(move |hit| {
        assert_eq!(hit.url, "/download/T-DOWN", "reset uploaded something");
        Reply::bytes(zip.clone())
    })
}

/// A workspace with local work and a conflict copy.
fn worked_on(dir: &Path) {
    let root = dir.join(SLIDE_DIR);
    std::fs::create_dir_all(root.join("src")).unwrap();
    std::fs::write(root.join("tally.py"), "count = 9\n").unwrap();
    std::fs::write(root.join("tally.ed-server.py"), "count = 5\n").unwrap();
    std::fs::write(root.join("mine.py"), "x\n").unwrap();
    let state = WorkspaceState {
        workspace_id: "W1".into(),
        slide_dir: SLIDE_DIR.into(),
        base_commit: Some("h1:::s".into()),
        files: [("tally.py".to_string(), sha256(b"count = 1\n"))].into(),
    };
    workspace_state::save(dir, 267880, &state).unwrap();
}

fn reset_paths(keyd: &FakeKeyd) -> Vec<(String, Vec<u8>)> {
    keyd.requests()
        .into_iter()
        .filter(|(h, _)| h["path"] == RESET)
        .map(|(h, body)| (h["method"].as_str().unwrap_or("").to_string(), body))
        .collect()
}

fn frames(out: &Receiver<Out>) -> Vec<String> {
    out.try_iter()
        .filter_map(|o| match o {
            Out::Frame(f) => Some(f),
            Out::Stop => None,
        })
        .collect()
}

#[test]
fn reset_takes_eds_tree_and_ends_terminals() {
    let dir = Scratch::new("ed-ws-reset");
    let keyd = fake_ed(&dir);
    let sahara = scaffold();
    worked_on(&dir);
    let (core, events) = context(&dir, &sahara.origin());
    go_online(&core);
    core.shared().user_id = Some(42);
    let opened = core.shared().terminals.open(PtySize { cols: 80, rows: 24 });
    let (session, out) = Session::detached(core.clone(), slide());

    session.reset(session.place()).unwrap();

    assert_eq!(reset_paths(&keyd), [("POST".to_string(), b"{}".to_vec())]);
    let root = dir.join(SLIDE_DIR);
    assert_eq!(
        std::fs::read(root.join("tally.py")).unwrap(),
        b"count = 0\n"
    );
    assert_eq!(std::fs::read(root.join("src/new.txt")).unwrap(), b"n\n");
    assert!(!root.join("mine.py").exists());
    assert!(!root.join("tally.ed-server.py").exists());

    let record = workspace_state::load(&dir, 267880).unwrap();
    assert_eq!(
        record.files,
        [
            ("src/new.txt".to_string(), sha256(b"n\n")),
            ("tally.py".to_string(), sha256(b"count = 0\n")),
        ]
        .into()
    );
    assert_eq!(record.base_commit, None);
    assert_eq!(record.workspace_id, "W1");

    assert_eq!(frames(&out), [terminal::kill_frame(&opened.id)]);
    let events: Vec<Body> = events
        .lock()
        .unwrap()
        .iter()
        .map(|e| e.body.clone())
        .collect();
    let ended = Body::TermState {
        id: opened.id.clone(),
        state: TermState::Exited,
    };
    assert!(events.contains(&ended), "{events:?}");
    assert!(events.iter().any(|b| matches!(b, Body::Files { .. })));
    assert_eq!(events.last(), Some(&Body::Reset));
    assert!(!core.shared().terminals.holds(&opened.id));
}

#[test]
fn reset_asks_ed_who_you_are_when_the_connect_did_not_say() {
    let dir = Scratch::new("ed-ws-reset-whoami");
    let keyd = fake_ed(&dir);
    let sahara = scaffold();
    worked_on(&dir);
    let (core, _) = context(&dir, &sahara.origin());
    go_online(&core);
    let (session, _out) = Session::detached(core, slide());
    session.reset(session.place()).unwrap();
    assert_eq!(reset_paths(&keyd).len(), 1);
}

#[test]
fn reset_needs_the_workspace_online() {
    let dir = Scratch::new("ed-ws-reset-offline");
    let keyd = fake_ed(&dir);
    worked_on(&dir);
    let (core, events) = context(&dir, "http://127.0.0.1:1");
    let (session, _out) = Session::detached(core, slide());
    let error = session.reset(session.place()).unwrap_err();
    assert!(error.contains("offline"), "{error}");
    assert!(reset_paths(&keyd).is_empty());
    assert!(events.lock().unwrap().is_empty());
    let root = dir.join(SLIDE_DIR);
    assert_eq!(
        std::fs::read(root.join("tally.py")).unwrap(),
        b"count = 9\n"
    );
}

#[test]
fn the_user_id_is_read_from_either_shape() {
    assert_eq!(user_id_in(&json!({"id": 42, "name": "Ada"})), Some(42));
    assert_eq!(user_id_in(&json!(42)), Some(42));
    assert_eq!(user_id_in(&json!("42")), Some(42));
    assert_eq!(user_id_in(&json!({"name": "Ada"})), None);
    assert_eq!(user_id_in(&Value::Null), None);
    assert_eq!(user_id_in(&json!(0)), None);
    assert_eq!(user_id_in(&json!({"id": 0})), None);
}

#[test]
fn the_reset_event_is_just_its_kind() {
    let event = serde_json::to_value(super::events::WorkspaceEvent {
        challenge_id: 7,
        body: Body::Reset,
    })
    .unwrap();
    assert_eq!(event, json!({"challengeId": 7, "kind": "reset"}));
}
