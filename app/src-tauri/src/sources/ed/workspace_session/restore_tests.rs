//! A submission's code and Restore, against a fake keyd (Ed's `/api`) and a
//! fake workspace server. The temporary workspace's socket is handed in.

use std::path::Path;

use serde_json::json;

use super::context::sha256;
use super::events::Body;
use super::frames;
use super::mark::code;
use super::session::Session;
use super::session_tests::{context, go_online, slide, zip_of, SLIDE_DIR};
use super::socket::Out;
use crate::sources::ed::workspace_state::{self, WorkspaceState};
use crate::sources::ed::Ed;
use crate::test_support::{FakeKeyd, FakeServer, Reply, Scratch};

fn fake_ed(dir: &Path, submission_id: i64) -> FakeKeyd {
    let connect = format!("/api/challenges/submissions/{submission_id}/connect");
    FakeKeyd::start(dir, move |header, _| {
        let path = header["path"].as_str().unwrap_or("");
        let body = if path == connect {
            json!({"ticket": "T-SUB"})
        } else {
            match path {
                "/api/workspaces/TMP/download" => json!({"ticket": "T-TMP"}),
                "/api/workspaces/W1/upload" => json!({"ticket": "T-UP"}),
                _ => json!({}),
            }
        };
        (
            json!({"status": 200, "headers": []}),
            body.to_string().into_bytes(),
        )
    })
}

/// Fetch submission `id` through a stand-in temporary workspace, which the
/// cache then holds.
fn fetch(ed: &Ed, id: i64) -> Vec<(String, Vec<u8>)> {
    let zip = zip_of(&[
        ("tally.py", b"count = 3\n"),
        ("same.py", b"s\n"),
        ("src/helper.py", b"h\n"),
        (".history", b"x"),
    ]);
    let files = code::fetch(
        ed,
        "sahara.test",
        id,
        |host, ticket| {
            assert_eq!((host, ticket.expose()), ("sahara.test", "T-SUB"));
            Ok(((), "TMP".to_string()))
        },
        |_, ticket| {
            assert_eq!(ticket.expose(), "T-TMP");
            Ok(zip)
        },
    )
    .unwrap();
    files
        .iter()
        .map(|f| (f.path.clone(), f.bytes.clone()))
        .collect()
}

#[test]
fn a_submissions_code_comes_from_its_temporary_copy_once() {
    let dir = Scratch::new("ed-submission-code");
    let keyd = fake_ed(&dir, 7101);
    let ed = Ed::open(&dir);
    let files = fetch(&ed, 7101);
    assert_eq!(
        files,
        [
            ("tally.py".to_string(), b"count = 3\n".to_vec()),
            ("same.py".to_string(), b"s\n".to_vec()),
            ("src/helper.py".to_string(), b"h\n".to_vec()),
        ]
    );
    let asked = keyd.requests().len();
    let again = code::files(&ed, 7101).unwrap();
    assert_eq!(code::as_text(&again)[0].text, "count = 3\n");
    assert_eq!(keyd.requests().len(), asked, "read from the cache");
    let paths: Vec<String> = keyd
        .requests()
        .iter()
        .map(|(h, _)| h["path"].as_str().unwrap_or("").to_string())
        .collect();
    assert!(!paths.iter().any(|p| p.contains("upload")), "{paths:?}");
}

#[test]
fn restore_replaces_the_workspace_here_and_on_ed() {
    let dir = Scratch::new("ed-ws-restore");
    let _keyd = fake_ed(&dir, 7102);
    let sahara = FakeServer::start(|hit| {
        assert_eq!(hit.url, "/upload/T-UP");
        Reply::json(json!({"hash": "up:::1"}))
    });
    let (core, events) = context(&dir, &sahara.origin());
    let root = dir.join(SLIDE_DIR);
    std::fs::write(root.join("tally.py"), "count = 9\n").unwrap();
    std::fs::write(root.join("tally.ed-server.py"), "count = 5\n").unwrap();
    std::fs::write(root.join("same.py"), "s\n").unwrap();
    std::fs::write(root.join("old.py"), "o\n").unwrap();
    std::fs::write(root.join("extra.txt"), "e\n").unwrap();
    let state = WorkspaceState {
        workspace_id: "W1".into(),
        slide_dir: SLIDE_DIR.into(),
        base_commit: Some("h1:::s".into()),
        files: [
            ("tally.py".to_string(), sha256(b"count = 1\n")),
            ("same.py".to_string(), sha256(b"s\n")),
            ("old.py".to_string(), sha256(b"o\n")),
        ]
        .into(),
    };
    workspace_state::save(&dir, 267880, &state).unwrap();
    fetch(&core.ed, 7102);
    let (session, out) = Session::detached(core.clone(), slide());

    let offline = session.restore(session.place(), 7102).unwrap_err();
    assert!(offline.contains("offline"), "{offline}");
    assert!(root.join("extra.txt").exists());

    go_online(&core);
    session.restore(session.place(), 7102).unwrap();

    let mut here = super::paths::list(&root);
    here.sort();
    assert_eq!(here, ["same.py", "src/helper.py", "tally.py"]);
    assert_eq!(
        std::fs::read(root.join("tally.py")).unwrap(),
        b"count = 3\n"
    );
    // Only what Ed lacks goes up; what Ed holds and the submission lacks goes.
    assert_eq!(sahara.hits().len(), 2);
    let sent: Vec<String> = out
        .try_iter()
        .filter_map(|o| match o {
            Out::Frame(f) => Some(f),
            Out::Stop => None,
        })
        .collect();
    assert_eq!(sent, [frames::remove("old.py", false)]);

    let record = workspace_state::load(&dir, 267880).unwrap();
    assert_eq!(record.files["tally.py"], sha256(b"count = 3\n"));
    assert_eq!(record.files["src/helper.py"], sha256(b"h\n"));
    assert_eq!(
        record.files["old.py"],
        sha256(b"o\n"),
        "kept until gone on Ed"
    );
    assert_eq!(record.base_commit.as_deref(), Some("up:::1"));

    let events: Vec<Body> = events
        .lock()
        .unwrap()
        .iter()
        .map(|e| e.body.clone())
        .collect();
    assert!(events.iter().any(|b| matches!(b, Body::Files { .. })));
    assert_eq!(events.last(), Some(&Body::Reset));
}
