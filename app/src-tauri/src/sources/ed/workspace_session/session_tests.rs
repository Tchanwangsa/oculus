//! The session against a fake keyd (Ed's `/api`) and a fake workspace
//! server. The socket thread needs Ed itself, and keyd serves Ed only to the
//! app and the CLI, so it is checked in the app.

use std::io::Write;
use std::path::Path;
use std::sync::{Arc, Mutex};

use serde_json::{json, Value};

use super::bootstrap;
use super::context::{sha256, Context, Unrecorded};
use super::events::{Body, Features, SavedTo, Sink, State, WorkspaceEvent};
use super::frames::{self, Init};
use super::session::Session;
use super::slide::Slide;
use super::socket::Out;
use crate::sources::ed::workspace_state;
use crate::sources::ed::Ed;
use crate::test_support::{FakeKeyd, FakeServer, Reply, Scratch};

pub(super) const SLIDE_DIR: &str = "courses/COMP30026/lessons/1-week-1/03-tally";

fn zip_of(entries: &[(&str, &[u8])]) -> Vec<u8> {
    let mut w = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    for (name, bytes) in entries {
        w.start_file(*name, zip::write::SimpleFileOptions::default())
            .unwrap();
        w.write_all(bytes).unwrap();
    }
    w.finish().unwrap().into_inner()
}

/// Ed's `/api` as the session calls it: tickets for every path.
pub(super) fn fake_ed(dir: &Path) -> FakeKeyd {
    FakeKeyd::start(dir, |header, _| {
        let body: Value = match header["path"].as_str().unwrap() {
            "/api/workspaces/W1/download" => json!({"ticket": "T-DOWN"}),
            "/api/workspaces/W1/upload" => json!({"ticket": "T-UP"}),
            "/api/challenges/267880/run" => json!({"ticket": "T-RUN"}),
            "/api/challenges/267880/mark" => json!({"submission": {"id": 9, "status": "failed"}}),
            "/api/challenges/submissions/9" => json!({"submission": {"id": 9}}),
            _ => json!({}),
        };
        (
            json!({"status": 200, "headers": []}),
            body.to_string().into_bytes(),
        )
    })
}

/// The workspace server: the zip on download, a fresh hash per upload.
fn fake_sahara(zip: Vec<u8>) -> FakeServer {
    let uploads = Mutex::new(0);
    FakeServer::start(move |hit| {
        if hit.url == "/download/T-DOWN" {
            return Reply::bytes(zip.clone());
        }
        let mut n = uploads.lock().unwrap();
        *n += 1;
        Reply::json(json!({"hash": format!("up{n}")}))
    })
}

pub(super) type Events = Arc<Mutex<Vec<WorkspaceEvent>>>;

pub(super) fn context(dir: &Path, origin: &str) -> (Arc<Context>, Events) {
    let events: Events = Arc::default();
    let log = events.clone();
    let sink: Sink = Arc::new(move |e| log.lock().unwrap().push(e));
    let mut context = Context::new(
        267880,
        dir.to_path_buf(),
        SLIDE_DIR.into(),
        Arc::new(Ed::open(dir)),
        sink,
    );
    context.sahara_origin = Some(origin.to_string());
    std::fs::create_dir_all(dir.join(SLIDE_DIR)).unwrap();
    (Arc::new(context), events)
}

pub(super) fn slide() -> Slide {
    Slide {
        slide_dir: SLIDE_DIR.into(),
        run_command: None,
        features: Features::default(),
        due_at: None,
    }
}

pub(super) fn go_online(context: &Context) {
    context.shared().online = Some(("W1".into(), "sahara.test".into()));
    context.set_status(State::Online, None);
}

fn init(head: &str) -> Init {
    frames::init(json!({"last_commits": [{"hash": head}]}))
}

#[test]
fn the_first_connect_records_eds_tree_and_keeps_what_was_saved_before_it() {
    let dir = Scratch::new("ed-ws-first-connect");
    let keyd = fake_ed(&dir);
    let sahara = fake_sahara(zip_of(&[
        ("tally.py", b"count = 0\n"),
        ("src/util.py", b"u\n"),
        ("gone.txt", b"g\n"),
        (".hidden", b"h"),
    ]));
    let (context, events) = context(&dir, &sahara.origin());
    let root = dir.join(SLIDE_DIR);
    // Saved and deleted here while the challenge had no record.
    std::fs::write(root.join("tally.py"), "count = 1\n").unwrap();
    {
        let mut shared = context.shared();
        shared
            .unrecorded
            .insert("tally.py".into(), Unrecorded::Saved);
        shared
            .unrecorded
            .insert("gone.txt".into(), Unrecorded::Deleted);
    }

    let pending = bootstrap::on_connect(&context, "W1", "sahara.test", &init("h0")).unwrap();

    assert_eq!(
        std::fs::read(root.join("tally.py")).unwrap(),
        b"count = 1\n"
    );
    assert_eq!(std::fs::read(root.join("src/util.py")).unwrap(), b"u\n");
    assert!(!root.join("gone.txt").exists());
    assert!(!root.join(".hidden").exists());
    assert_eq!(pending, [frames::remove("gone.txt", false)]);

    let record = workspace_state::load(&dir, 267880).unwrap();
    assert_eq!(record.workspace_id, "W1");
    assert_eq!(record.slide_dir, SLIDE_DIR);
    // The local save went up and became the base; the removed file stays
    // recorded until a reconcile finds it gone from Ed.
    assert_eq!(record.base_commit.as_deref(), Some("up1"));
    assert_eq!(
        record.files.into_iter().collect::<Vec<_>>(),
        [
            ("gone.txt".to_string(), sha256(b"g\n")),
            ("src/util.py".to_string(), sha256(b"u\n")),
            ("tally.py".to_string(), sha256(b"count = 1\n")),
        ]
    );
    assert!(context.shared().unrecorded.is_empty());

    let hits = sahara.hits();
    assert_eq!(hits[0].method, "GET");
    assert_eq!(hits[1].url, "/upload/T-UP");
    let body = String::from_utf8_lossy(&hits[1].body);
    assert!(
        body.contains("name=\"file\"; filename=\"tally.py\""),
        "{body}"
    );
    assert!(body.contains("count = 1\n"), "{body}");
    let requests = keyd.requests();
    assert_eq!(
        serde_json::from_slice::<Value>(&requests[0].1).unwrap(),
        json!({"path": "."})
    );
    assert_eq!(
        serde_json::from_slice::<Value>(&requests[1].1).unwrap(),
        json!({"wid": "W1", "path": "."})
    );
    assert!(events.lock().unwrap().contains(&WorkspaceEvent {
        challenge_id: 267880,
        body: Body::Files {
            files: vec!["src/util.py".into(), "tally.py".into()]
        }
    }));

    // With a record, a connect reconciles. Ed (the fake) still serves its
    // first tree: `tally.py` changed on both sides, `gone.txt` is still there.
    std::fs::write(root.join("tally.py"), "count = 2\n").unwrap();
    let pending = bootstrap::on_connect(&context, "W1", "sahara.test", &init("h9")).unwrap();
    assert_eq!(pending, [frames::remove("gone.txt", false)]);
    assert_eq!(
        std::fs::read(root.join("tally.py")).unwrap(),
        b"count = 2\n"
    );
    assert_eq!(
        std::fs::read(root.join("tally.ed-server.py")).unwrap(),
        b"count = 0\n"
    );
    assert!(events.lock().unwrap().contains(&WorkspaceEvent {
        challenge_id: 267880,
        body: Body::Conflict {
            path: "tally.py".into(),
            server_path: "tally.ed-server.py".into(),
        }
    }));
    let record = workspace_state::load(&dir, 267880).unwrap();
    assert_eq!(record.base_commit.as_deref(), Some("h9"));
    assert_eq!(record.files["tally.py"], sha256(b"count = 1\n"));
    // One download, one upload, then the second download; nothing uploaded.
    assert_eq!(sahara.hits().len(), 3);

    // Ed still at the base: no download.
    let pending = bootstrap::on_connect(&context, "W1", "sahara.test", &init("h9")).unwrap();
    assert_eq!(pending, [frames::remove("gone.txt", false)]);
    assert_eq!(sahara.hits().len(), 3);
}

#[test]
fn the_first_connect_uploads_new_local_files_and_drops_old_sync_names() {
    let dir = Scratch::new("ed-ws-first-leftovers");
    let _keyd = fake_ed(&dir);
    let sahara = fake_sahara(zip_of(&[("my file.py", b"m"), ("a.py", b"a")]));
    let (context, _) = context(&dir, &sahara.origin());
    let root = dir.join(SLIDE_DIR);
    // The old sync's name for `my file.py`, and a file Ed never had.
    std::fs::write(root.join("my_file.py"), "m").unwrap();
    std::fs::write(root.join("notes.txt"), "n").unwrap();

    bootstrap::on_connect(&context, "W1", "sahara.test", &init("h0")).unwrap();

    assert!(!root.join("my_file.py").exists());
    assert_eq!(std::fs::read(root.join("my file.py")).unwrap(), b"m");
    assert_eq!(std::fs::read(root.join("notes.txt")).unwrap(), b"n");
    let record = workspace_state::load(&dir, 267880).unwrap();
    assert_eq!(
        record.files.keys().collect::<Vec<_>>(),
        ["a.py", "my file.py", "notes.txt"]
    );
    assert_eq!(record.base_commit.as_deref(), Some("up1"));
    let body = String::from_utf8_lossy(&sahara.hits()[1].body).into_owned();
    assert!(body.contains("filename=\"notes.txt\""), "{body}");

    let old = bootstrap::old_sync_names(["my file.py", "a.py", "dir/x y.txt"].into_iter());
    assert_eq!(
        old.into_iter().collect::<Vec<_>>(),
        ["dir/x_y.txt", "my_file.py"]
    );
}

#[test]
fn an_online_save_uploads_and_moves_the_base() {
    let dir = Scratch::new("ed-ws-save-online");
    let _keyd = fake_ed(&dir);
    let sahara = fake_sahara(Vec::new());
    let (context, events) = context(&dir, &sahara.origin());
    go_online(&context);
    let (session, _out) = Session::detached(context.clone(), slide());

    let saved = session
        .save(session.place(), "src/a.py", b"print(1)\n")
        .unwrap();
    assert_eq!(saved.saved, SavedTo::Ed);
    assert_eq!(saved.hash.as_deref(), Some("up1"));
    assert_eq!(session.read("src/a.py").unwrap(), "print(1)\n");
    let record = workspace_state::load(&dir, 267880).unwrap();
    assert_eq!(record.base_commit.as_deref(), Some("up1"));
    assert_eq!(record.files["src/a.py"], sha256(b"print(1)\n"));
    assert_eq!(sahara.hits()[0].url, "/upload/T-UP");

    let events = events.lock().unwrap();
    let kinds: Vec<&Body> = events.iter().map(|e| &e.body).collect();
    assert!(kinds.contains(&&Body::Files {
        files: vec!["src/a.py".into()]
    }));
    drop(events);

    // A frame for our hash after the answer is ours.
    assert!(context.shared().commits.arrived("up1"));
}

#[test]
fn a_failed_upload_is_an_error_after_the_file_is_written() {
    let dir = Scratch::new("ed-ws-save-fails");
    let _keyd = fake_ed(&dir);
    let sahara = FakeServer::start(|_| Reply::status(500, json!({})));
    let (context, _) = context(&dir, &sahara.origin());
    go_online(&context);
    let (session, _out) = Session::detached(context, slide());
    let error = session.save(session.place(), "tally.py", b"x").unwrap_err();
    assert!(error.contains("HTTP 500"), "{error}");
    assert!(!error.contains("T-UP"), "{error}");
    assert_eq!(session.read("tally.py").unwrap(), "x");
    assert!(!workspace_state::has_state(&dir, 267880));
}

#[test]
fn offline_changes_stay_local_and_are_remembered_until_a_record_exists() {
    let dir = Scratch::new("ed-ws-offline");
    let (context, events) = context(&dir, "http://127.0.0.1:1");
    context.set_status(State::Offline, Some("no network".into()));
    let (session, out) = Session::detached(context.clone(), slide());

    let saved = session.save(session.place(), "tally.py", b"a").unwrap();
    assert_eq!((saved.saved, saved.hash), (SavedTo::Local, None));
    session.delete(session.place(), "tally.py").unwrap();
    assert!(!dir.join(SLIDE_DIR).join("tally.py").exists());
    assert_eq!(
        context.shared().unrecorded.get("tally.py"),
        Some(&Unrecorded::Deleted)
    );
    assert!(out.try_recv().is_err(), "nothing is sent while offline");
    assert!(session.save(session.place(), "../x", b"a").is_err());

    // With a record, offline changes are left for reconcile.
    workspace_state::save(&dir, 267880, &workspace_state::WorkspaceState::default()).unwrap();
    session.save(session.place(), "b.py", b"b").unwrap();
    assert_eq!(context.shared().unrecorded.get("b.py"), None);

    let files: Vec<Vec<String>> = events
        .lock()
        .unwrap()
        .iter()
        .filter_map(|e| match &e.body {
            Body::Files { files } => Some(files.clone()),
            _ => None,
        })
        .collect();
    assert_eq!(
        files,
        [
            vec!["tally.py".to_string()],
            vec![],
            vec!["b.py".to_string()]
        ]
    );
}

#[test]
fn an_online_delete_sends_the_remove_and_keeps_the_record_for_reconcile() {
    let dir = Scratch::new("ed-ws-delete");
    let (context, _) = context(&dir, "http://127.0.0.1:1");
    let root = dir.join(SLIDE_DIR);
    std::fs::create_dir_all(root.join("src")).unwrap();
    std::fs::write(root.join("src/a.py"), "a").unwrap();
    std::fs::write(root.join("keep.py"), "k").unwrap();
    let mut state = workspace_state::WorkspaceState::default();
    for p in ["src/a.py", "keep.py", "srcfile.py"] {
        state.files.insert(p.into(), "x".into());
    }
    workspace_state::save(&dir, 267880, &state).unwrap();
    go_online(&context);
    let (session, out) = Session::detached(context, slide());

    session.delete(session.place(), "src").unwrap();
    assert!(!root.join("src").exists());
    match out.try_recv() {
        Ok(Out::Frame(frame)) => assert_eq!(frame, frames::remove("src", true)),
        _ => panic!("no remove frame"),
    }
    // Forgotten only once a reconcile finds it gone from Ed too.
    let record = workspace_state::load(&dir, 267880).unwrap();
    assert_eq!(
        record.files.keys().collect::<Vec<_>>(),
        ["keep.py", "src/a.py", "srcfile.py"]
    );
}

#[test]
fn the_run_mark_and_submission_calls_post_what_ed_expects() {
    let dir = Scratch::new("ed-ws-api");
    let keyd = fake_ed(&dir);
    let ed = Ed::open(&dir);
    let ticket = ed
        .run_ticket(267880, "h1", "python3 tally.py", (80, 24))
        .unwrap();
    assert_eq!(ticket.expose(), "T-RUN");
    assert_eq!(format!("{ticket:?}"), "Ticket(…)");
    assert_eq!(ed.mark(267880, "h1").unwrap()["status"], "failed");
    assert_eq!(ed.submission(9).unwrap()["id"], 9);
    assert!(ed.upload_ticket("W2").is_err());

    let requests = keyd.requests();
    let body = |i: usize| serde_json::from_slice::<Value>(&requests[i].1).unwrap();
    assert_eq!(
        body(0),
        json!({"pty_size": {"cols": 80, "rows": 24}, "hash": "h1",
               "command": "python3 tally.py", "dump_files": true})
    );
    assert_eq!(body(1), json!({"hash": "h1"}));
    assert_eq!(requests[2].0["method"], "GET");
    assert_eq!(requests[2].0["path"], "/api/challenges/submissions/9");
}
