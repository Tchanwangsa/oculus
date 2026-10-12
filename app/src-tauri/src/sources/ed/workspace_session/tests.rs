use std::sync::{Arc, Mutex};

use serde_json::{json, Value};

use super::commits::Commits;
use super::events::{Body, Features, Saved, SavedTo, State, Status, WorkspaceEvent, WorkspaceInfo};
use super::frames::{self, Incoming};
use super::paths::{list, resolve};
use super::queue::Queue;
use super::slide;
use crate::sources::ed::front::front_matter;
use crate::sources::ed::workspace_state::{self, WorkspaceState};
use crate::test_support::Scratch;

#[test]
fn init_keeps_the_head_commit_and_never_the_auth() {
    let text = json!({
        "type": "init",
        "data": {
            "read_only": false, "temporary": false, "id": 7, "wid": "W1",
            "web_id": "x", "auth": "SECRET-AUTH", "files_open": ["tally.py"],
            "last_commits": [{"hash": "h2", "at": "2026-10-13T00:00:00Z"}, {"hash": "h1", "at": "x"}],
            "program_state": {"term-1": {"state": "running"}},
        }
    })
    .to_string();
    let Some(Incoming::Init(init)) = frames::parse(&text) else {
        panic!("not an init");
    };
    assert_eq!(init.head(), Some("h2"));
    assert!(!init.read_only);
    assert_eq!(init.program_state["term-1"]["state"], "running");
    assert!(!format!("{init:?}").contains("SECRET"));

    // As `live::wait_for_init` hands over the data alone.
    let bare = frames::init(json!({"last_commits": [], "auth": "SECRET-AUTH"}));
    assert_eq!(bare.head(), None);
    assert_eq!(frames::init(json!("nonsense")), frames::Init::default());
}

#[test]
fn the_frames_a_workspace_uses_are_read_and_the_rest_ignored() {
    let parse = |v: Value| frames::parse(&v.to_string());
    let Some(Incoming::NewCommit(c)) = parse(json!({"type": "new_commit",
        "data": {"commit": {"hash": "abc", "at": "t"}, "context": null}}))
    else {
        panic!()
    };
    assert_eq!((c.commit.hash.as_str(), c.context), ("abc", None));
    let Some(Incoming::NewCommit(c)) = parse(json!({"type": "new_commit",
        "data": {"commit": {"hash": "def"}, "context": {"id": 3}}}))
    else {
        panic!()
    };
    assert_eq!(c.context, Some(json!({"id": 3})));

    let Some(Incoming::Fsop(op)) = parse(json!({"type": "fsop",
        "data": {"type": "remove", "param1": "dir/x.txt", "param2": "", "param3": ""}}))
    else {
        panic!()
    };
    assert_eq!(
        (op.op.as_str(), op.param1.as_deref()),
        ("remove", Some("dir/x.txt"))
    );

    let Some(Incoming::ListReply(list)) = parse(json!({"type": "list_reply", "data": {"listing": [
        {"name": "tally.py", "type": "file", "created_at": "a", "modified_at": "b"},
        {"name": "dir", "type": "folder", "created_at": "a", "modified_at": "b"},
    ]}})) else {
        panic!()
    };
    assert_eq!(list.listing[1].name, "dir");
    assert_eq!(list.listing[1].kind, "folder");

    assert_eq!(
        parse(json!({"type": "display_error", "data": "Disk full"})),
        Some(Incoming::DisplayError("Disk full".into()))
    );
    assert_eq!(
        parse(json!({"type": "display_error", "data": {"message": "No"}})),
        Some(Incoming::DisplayError("No".into()))
    );
    let Some(Incoming::ProgramData(data)) =
        parse(json!({"type": "program_data", "data": {"id": "term-1", "data": "aGk="}}))
    else {
        panic!()
    };
    assert_eq!((data.id.as_str(), data.data.as_str()), ("term-1", "aGk="));
    let Some(Incoming::ProgramInit(p)) = parse(json!({"type": "program_init",
        "data": {"id": "term-1", "state": "running", "command": "bash --login", "persist": true}}))
    else {
        panic!()
    };
    assert!(p.persist);
    assert_eq!(
        parse(json!({"type": "program_close", "data": "term-1"})),
        Some(Incoming::ProgramClose(frames::ProgramClose {
            id: "term-1".into()
        }))
    );
    assert_eq!(parse(json!({"type": "ping"})), Some(Incoming::Ping));

    // Live editing, chat and the rest are not ours; nor is a malformed known kind.
    assert_eq!(parse(json!({"type": "file_ot", "data": {"fid": 1}})), None);
    assert_eq!(
        parse(json!({"type": "new_commit", "data": {"nope": 1}})),
        None
    );
    assert_eq!(frames::parse("not json"), None);
}

#[test]
fn outgoing_frames_are_type_and_data() {
    let keep: Value = serde_json::from_str(&frames::keep_alive()).unwrap();
    assert_eq!(keep, json!({"type": "keep_alive"}));
    let remove: Value = serde_json::from_str(&frames::remove("src/a.py")).unwrap();
    assert_eq!(
        remove,
        json!({"type": "fsop", "data": {"type": "remove", "param1": "src/a.py", "param2": "", "param3": ""}})
    );
}

#[test]
fn a_commit_is_ours_when_our_upload_named_it_whichever_comes_first() {
    let mut c = Commits::default();
    // The HTTP answer first, then the frame.
    c.begin();
    assert_eq!(c.finish(Some("h1")), []);
    assert_eq!(c.arrived("h1"), Some(("h1".into(), true)));

    // The frame first: held until the upload answers.
    c.begin();
    assert_eq!(c.arrived("h2"), None);
    assert_eq!(c.finish(Some("h2")), [("h2".to_string(), true)]);

    // Another client's commit during an upload waits for it, then is theirs.
    c.begin();
    assert_eq!(c.arrived("theirs"), None);
    assert_eq!(c.arrived("h3"), None);
    assert_eq!(
        c.finish(Some("h3")),
        [("h3".to_string(), true), ("theirs".to_string(), false)]
    );

    // With nothing in flight, an unknown commit is decided at once.
    assert_eq!(c.arrived("web"), Some(("web".into(), false)));

    // A failed upload releases what it held.
    c.begin();
    assert_eq!(c.arrived("x"), None);
    assert_eq!(c.finish(None), [("x".to_string(), false)]);
}

#[test]
fn the_guard_keeps_every_path_inside_the_workspace() {
    let dir = Scratch::new("ed-ws-guard");
    let root = dir.join("ws");
    std::fs::create_dir_all(root.join("src")).unwrap();
    std::fs::write(root.join("tally.py"), "x").unwrap();

    assert_eq!(resolve(&root, "tally.py").unwrap(), root.join("tally.py"));
    assert_eq!(
        resolve(&root, "src/new/a.py").unwrap(),
        root.join("src/new/a.py")
    );
    for bad in [
        "",
        "/etc/passwd",
        "../x.py",
        "src/../../x",
        "./a.py",
        "a//b",
        "src/",
        ".env",
        "src/.git/config",
        "a\\b",
        "C:x",
        "a\0b",
    ] {
        assert!(resolve(&root, bad).is_err(), "{bad:?} was let through");
    }
    assert!(resolve(&dir.join("missing"), "a.py").is_err());

    #[cfg(unix)]
    {
        let outside = dir.join("outside");
        std::fs::create_dir_all(&outside).unwrap();
        std::fs::write(outside.join("secret.txt"), "s").unwrap();
        std::os::unix::fs::symlink(&outside, root.join("link")).unwrap();
        std::os::unix::fs::symlink(outside.join("secret.txt"), root.join("s.txt")).unwrap();
        std::os::unix::fs::symlink(root.join("tally.py"), root.join("inside.py")).unwrap();
        assert!(resolve(&root, "link/secret.txt").is_err());
        assert!(resolve(&root, "link/new.txt").is_err());
        assert!(resolve(&root, "s.txt").is_err());
        assert!(resolve(&root, "inside.py").is_ok());
    }
}

#[test]
fn the_file_list_is_sorted_and_leaves_out_hidden_files_and_links() {
    let dir = Scratch::new("ed-ws-list");
    std::fs::create_dir_all(dir.join("src/.cache")).unwrap();
    for f in [
        "tally.py",
        "src/b.py",
        "src/a.py",
        ".env",
        "src/.cache/x",
        ".tally.py.oculus-1-2",
    ] {
        std::fs::write(dir.join(f), "x").unwrap();
    }
    #[cfg(unix)]
    std::os::unix::fs::symlink(dir.join("tally.py"), dir.join("link.py")).unwrap();
    assert_eq!(list(&dir), ["src/a.py", "src/b.py", "tally.py"]);
    assert!(list(&dir.join("missing")).is_empty());
}

#[test]
fn changes_take_their_turns_in_the_order_asked() {
    let queue = Arc::new(Queue::default());
    let done = Arc::new(Mutex::new(Vec::new()));
    let places: Vec<_> = (0..5).map(|_| queue.place()).collect();
    let mut threads = Vec::new();
    // The last place starts waiting first: it must still go last.
    for (i, place) in places.into_iter().enumerate().rev() {
        let done = done.clone();
        threads.push(std::thread::spawn(move || {
            let _turn = place.wait();
            done.lock().unwrap().push(i);
        }));
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    for t in threads {
        t.join().unwrap();
    }
    assert_eq!(*done.lock().unwrap(), [0, 1, 2, 3, 4]);

    // A place given up unused lets the next one through.
    let skipped = queue.place();
    let next = queue.place();
    drop(skipped);
    drop(next.wait());
}

/// A slide file as the sync writes it, for challenge `id`.
fn write_slide(dir: &std::path::Path, md: &str, id: i64, challenge: Value) {
    let front = front_matter(&[
        ("ed_slide_id", json!(1)),
        ("ed_challenge_id", json!(id)),
        ("title", json!("Tally")),
        ("type", json!("code")),
        ("challenge", challenge),
    ]);
    let path = dir.join(md);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, front + "# Tally\n").unwrap();
}

#[test]
fn a_challenge_is_found_by_its_slide_front_matter() {
    let dir = Scratch::new("ed-ws-slide");
    let md = "courses/COMP30026/lessons/1-week-1/03-tally.md";
    write_slide(
        &dir,
        "courses/COMP30026/lessons/1-week-1/02-other.md",
        2678800,
        Value::Null,
    );
    write_slide(
        &dir,
        md,
        267880,
        json!({"type": "none", "run_command": "python3 tally.py",
               "features": {"run": true, "mark": false, "terminal": true, "feedback": null,
                            "confirm_submit": true},
               "due_at": "2026-10-20T00:00:00Z"}),
    );
    let found = slide::find(&dir, 267880).unwrap();
    assert_eq!(
        found.slide_dir,
        "courses/COMP30026/lessons/1-week-1/03-tally"
    );
    assert_eq!(found.run_command.as_deref(), Some("python3 tally.py"));
    assert_eq!(
        found.features,
        Features {
            run: true,
            mark: false,
            terminal: true,
            feedback: false,
            confirm_submit: true
        }
    );
    assert_eq!(found.due_at.as_deref(), Some("2026-10-20T00:00:00Z"));

    // No snapshot: every feature off, no command.
    let bare = slide::find(&dir, 2678800).unwrap();
    assert_eq!(
        (bare.run_command, bare.features),
        (None, Features::default())
    );
    assert!(slide::find(&dir, 1).is_err());

    // The record's folder is taken when its slide still names the challenge.
    let moved = "courses/COMP30026/lessons/9-moved/01-tally.md";
    write_slide(&dir, moved, 267880, Value::Null);
    let state = WorkspaceState {
        slide_dir: "courses/COMP30026/lessons/9-moved/01-tally".into(),
        ..WorkspaceState::default()
    };
    workspace_state::save(&dir, 267880, &state).unwrap();
    assert_eq!(
        slide::find(&dir, 267880).unwrap().slide_dir,
        state.slide_dir
    );
}

#[test]
fn events_and_results_have_the_panes_shapes() {
    let event = |body| {
        serde_json::to_value(WorkspaceEvent {
            challenge_id: 5,
            body,
        })
        .unwrap()
    };
    assert_eq!(
        event(Body::Status {
            status: Status {
                state: State::Offline,
                reason: Some("gone".into())
            }
        }),
        json!({"challengeId": 5, "kind": "status", "status": {"state": "offline", "reason": "gone"}})
    );
    assert_eq!(
        event(Body::Commit {
            hash: "h".into(),
            ours: true
        }),
        json!({"challengeId": 5, "kind": "commit", "hash": "h", "ours": true})
    );
    assert_eq!(
        event(Body::Files {
            files: vec!["a.py".into()]
        }),
        json!({"challengeId": 5, "kind": "files", "files": ["a.py"]})
    );
    assert_eq!(
        event(Body::Error {
            message: "m".into()
        }),
        json!({"challengeId": 5, "kind": "error", "message": "m"})
    );
    let info = WorkspaceInfo {
        challenge_id: 5,
        slide_dir: "courses/C/lessons/1-l/01-s".into(),
        files: vec!["a.py".into()],
        run_command: None,
        features: Features {
            confirm_submit: true,
            ..Features::default()
        },
        due_at: None,
        status: Status::connecting(),
    };
    assert_eq!(
        serde_json::to_value(info).unwrap(),
        json!({"challengeId": 5, "slideDir": "courses/C/lessons/1-l/01-s", "files": ["a.py"],
               "runCommand": null,
               "features": {"run": false, "mark": false, "terminal": false, "feedback": false,
                            "confirmSubmit": true},
               "dueAt": null, "status": {"state": "connecting", "reason": null}})
    );
    assert_eq!(
        serde_json::to_value(Saved {
            saved: SavedTo::Local,
            hash: None
        })
        .unwrap(),
        json!({"saved": "local", "hash": null})
    );
}
