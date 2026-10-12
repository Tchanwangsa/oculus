//! Settling a conflict and pulling, against a fake keyd (Ed's `/api`) and a
//! fake workspace server.

use std::path::Path;
use std::sync::{Arc, Mutex};

use serde_json::json;

use super::pull::Puller;
use super::resolve::{resolve, Keep};
use crate::sources::ed::workspace_session::context::{sha256, Context};
use crate::sources::ed::workspace_session::events::{Body, Sink, State, WorkspaceEvent};
use crate::sources::ed::workspace_state::{self, WorkspaceState};
use crate::sources::ed::Ed;
use crate::test_support::{FakeKeyd, FakeServer, Reply, Scratch};

const SLIDE_DIR: &str = "courses/COMP30026/lessons/1-week-1/03-tally";

fn fake_ed(dir: &Path) -> FakeKeyd {
    FakeKeyd::start(dir, |_, _| {
        (
            json!({"status": 200, "headers": []}),
            json!({"ticket": "T-UP"}).to_string().into_bytes(),
        )
    })
}

fn context(dir: &Path, origin: &str) -> Arc<Context> {
    let sink: Sink = Arc::new(|_| {});
    let mut context = Context::new(
        267880,
        dir.to_path_buf(),
        SLIDE_DIR.into(),
        Arc::new(Ed::open(dir)),
        sink,
    );
    context.sahara_origin = Some(origin.to_string());
    let root = dir.join(SLIDE_DIR);
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(root.join("a.py"), "mine").unwrap();
    std::fs::write(root.join("a.ed-server.py"), "theirs").unwrap();
    let state = WorkspaceState {
        workspace_id: "W1".into(),
        base_commit: Some("h0".into()),
        files: [("a.py".to_string(), sha256(b"base"))].into(),
        ..WorkspaceState::default()
    };
    workspace_state::save(dir, 267880, &state).unwrap();
    Arc::new(context)
}

fn uploads() -> FakeServer {
    FakeServer::start(|_| Reply::json(json!({"hash": "up1"})))
}

#[test]
fn keep_mine_offline_makes_eds_version_the_base() {
    let dir = Scratch::new("ed-resolve-mine-offline");
    let sahara = uploads();
    let core = context(&dir, &sahara.origin());
    resolve(&core, "a.py", Keep::Mine).unwrap();

    let root = dir.join(SLIDE_DIR);
    assert!(!root.join("a.ed-server.py").exists());
    assert_eq!(std::fs::read(root.join("a.py")).unwrap(), b"mine");
    let record = workspace_state::load(&dir, 267880).unwrap();
    assert_eq!(record.files["a.py"], sha256(b"theirs"));
    assert!(sahara.hits().is_empty());
}

#[test]
fn keep_mine_online_uploads_it() {
    let dir = Scratch::new("ed-resolve-mine-online");
    let _keyd = fake_ed(&dir);
    let sahara = uploads();
    let core = context(&dir, &sahara.origin());
    core.shared().online = Some(("W1".into(), "sahara.test".into()));
    core.set_status(State::Online, None);
    resolve(&core, "a.py", Keep::Mine).unwrap();

    let record = workspace_state::load(&dir, 267880).unwrap();
    assert_eq!(record.files["a.py"], sha256(b"mine"));
    assert_eq!(record.base_commit.as_deref(), Some("up1"));
    assert_eq!(sahara.hits()[0].url, "/upload/T-UP");
}

#[test]
fn take_eds_replaces_the_file_and_unrecords_it() {
    let dir = Scratch::new("ed-resolve-ed");
    let core = context(&dir, "http://127.0.0.1:1");
    resolve(&core, "a.py", Keep::Ed).unwrap();

    let root = dir.join(SLIDE_DIR);
    assert_eq!(std::fs::read(root.join("a.py")).unwrap(), b"theirs");
    assert!(!root.join("a.ed-server.py").exists());
    let record = workspace_state::load(&dir, 267880).unwrap();
    assert!(!record.files.contains_key("a.py"));

    let error = resolve(&core, "a.py", Keep::Ed).unwrap_err();
    assert!(error.contains("no copy"), "{error}");
}

#[test]
fn commits_from_another_client_coalesce_into_one_owed_pull() {
    let puller = Puller::default();
    assert!(!puller.take_owed());
    puller.want("c1");
    puller.want("c2");
    assert!(puller.take_owed());
    assert!(!puller.take_owed());
    puller.owe();
    assert!(puller.take_owed());
}

#[test]
fn another_clients_commit_wants_a_pull_and_ours_does_not() {
    let dir = Scratch::new("ed-pull-wanted");
    let events: Arc<Mutex<Vec<WorkspaceEvent>>> = Arc::default();
    let log = events.clone();
    let sink: Sink = Arc::new(move |e| log.lock().unwrap().push(e));
    let core = Context::new(
        267880,
        dir.to_path_buf(),
        SLIDE_DIR.into(),
        Arc::new(Ed::open(&dir)),
        sink,
    );

    core.shared().commits.begin();
    core.commit_arrived("snapshot".into());
    assert!(!core.puller.take_owed());
    core.shared().commits.finish(Some("up1"));
    core.commit_arrived("web".into());
    assert!(core.puller.take_owed());
    assert_eq!(
        events
            .lock()
            .unwrap()
            .iter()
            .map(|e| e.body.clone())
            .collect::<Vec<_>>(),
        [
            Body::Commit {
                hash: "snapshot".into(),
                ours: true
            },
            Body::Commit {
                hash: "web".into(),
                ours: false
            },
        ]
    );
}

#[test]
fn a_conflict_event_has_the_panes_shape() {
    let event = WorkspaceEvent {
        challenge_id: 5,
        body: Body::Conflict {
            path: "a.py".into(),
            server_path: "a.ed-server.py".into(),
        },
    };
    assert_eq!(
        serde_json::to_value(event).unwrap(),
        json!({"challengeId": 5, "kind": "conflict", "path": "a.py", "serverPath": "a.ed-server.py"})
    );
}
