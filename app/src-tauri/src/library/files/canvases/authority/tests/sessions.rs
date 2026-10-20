use super::super::error::Code;
use super::super::events::{Event, Phase};
use super::super::session::{Op, Resolution};
use super::super::{format, model, receipts, Authority};
use super::Rows;
use crate::test_support::Scratch;
use serde_json::{json, Value};
use std::sync::{Arc, Mutex};

const FILE: i64 = 7;

fn shape(id: &str, x: f64) -> Value {
    json!({"id": id, "type": "rectangle", "x": x, "y": 0, "width": 40, "height": 30,
        "stroke": "#000", "fill": "transparent", "strokeWidth": 2, "style": "clean", "seed": 1,
        "plugin": {"kept": true}})
}

fn add(id: &str) -> Value {
    json!({"edits": [{"id": id, "before": null, "after": shape(id, 0.0)}]})
}

fn op(client: &str, seq: u64, transaction: Value) -> Op {
    Op {
        client_id: client.into(),
        seq,
        op_id: format!("{client}-{seq}"),
        transaction,
        viewport: None,
        epoch: None,
    }
}

struct Fixture {
    root: Scratch,
    authority: Authority,
    rows: Arc<Rows>,
    events: Arc<Mutex<Vec<Event>>>,
    rel: String,
    text: String,
}

impl Fixture {
    fn new(name: &str) -> Self {
        Self::with(
            name,
            json!({"type": "oculus-canvas", "version": 1, "elements": [shape("a", 0.0)],
            "viewport": {"x": 0, "y": 0, "zoom": 1}, "background": "#ffffff", "meta": {"kept": 1}}),
        )
    }

    fn with(name: &str, document: Value) -> Self {
        let root = Scratch::new(name);
        let text = format::serialize(&document).unwrap();
        let rel = super::super::super::create_at(&root, "TEST", "Shared", &text)
            .unwrap()
            .relative_path;
        let rows = Arc::new(Rows::default());
        let authority = Authority::new(rows.clone());
        let events = Arc::new(Mutex::new(Vec::new()));
        let sink = events.clone();
        authority
            .subscribers()
            .subscribe(move |event| sink.lock().unwrap().push(event.clone()));
        Self {
            root,
            authority,
            rows,
            events,
            rel,
            text,
        }
    }

    fn open(&self) -> super::super::session::Snapshot {
        self.authority
            .open(FILE, &self.root, &self.rel, &self.text)
            .unwrap()
    }

    fn disk(&self) -> String {
        std::fs::read_to_string(self.root.join(&self.rel)).unwrap()
    }

    fn changes(&self) -> Vec<(u64, Option<String>)> {
        self.events
            .lock()
            .unwrap()
            .iter()
            .filter_map(|event| match event {
                Event::Change(c) => Some((c.revision, c.client_id.clone())),
                Event::State(_) => None,
            })
            .collect()
    }

    fn phases(&self) -> Vec<Phase> {
        self.events
            .lock()
            .unwrap()
            .iter()
            .filter_map(|event| match event {
                Event::State(s) => Some(s.phase),
                Event::Change(_) => None,
            })
            .collect()
    }
}

fn ids(text: &str) -> Vec<String> {
    let value: Value = serde_json::from_str(text).unwrap();
    model::validate(&value).unwrap();
    value["elements"]
        .as_array()
        .unwrap()
        .iter()
        .map(|el| el["id"].as_str().unwrap().to_string())
        .collect()
}

#[test]
fn a_commit_persists_bumps_the_revision_and_broadcasts() {
    let f = Fixture::new("authority-commit");
    let opened = f.open();
    assert_eq!(
        (opened.revision, opened.text.as_str()),
        (0, f.text.as_str())
    );
    assert!(!opened.paused);
    let ack = f.authority.commit(FILE, op("ipad", 1, add("b"))).unwrap();
    assert_eq!(
        (ack.revision, ack.epoch.as_str(), ack.replayed),
        (1, opened.epoch.as_str(), false)
    );
    assert_eq!(ack.transaction.unwrap()["edits"][0]["id"], "b");
    let disk = f.disk();
    assert_eq!(ids(&disk), ["a", "b"]);
    assert!(disk.contains("\"meta\": {\n    \"kept\": 1\n  }"));
    assert!(disk.ends_with("}\n"));
    assert_eq!(disk, f.authority.snapshot(FILE).unwrap().text);
    assert_eq!(f.changes(), [(1, Some("ipad".to_string()))]);
    assert_eq!(f.rows.log(), [format!("touch {FILE} {}", disk.len())]);
}

#[test]
fn open_refuses_text_that_is_not_the_file_on_disk() {
    let f = Fixture::new("authority-open-mismatch");
    let error = f
        .authority
        .open(FILE, &f.root, &f.rel, &f.text.replace("#ffffff", "#000000"))
        .unwrap_err();
    assert_eq!(error.code, Code::ExternalEdit);
    assert_eq!(
        f.authority
            .commit(FILE, op("c", 1, add("b")))
            .unwrap_err()
            .code,
        Code::SessionEnded
    );
    let other = f.root.join("courses/TEST/canvases/Other.canvas");
    std::fs::write(&other, "{}").unwrap();
    let error = f
        .authority
        .open(FILE, &f.root, "courses/TEST/canvases/Other.canvas", "{}")
        .unwrap_err();
    assert_eq!(error.code, Code::Invalid);
}

#[test]
fn a_second_open_attaches_to_the_live_session() {
    let f = Fixture::new("authority-reopen");
    let first = f.open();
    f.authority.commit(FILE, op("c", 1, add("b"))).unwrap();
    let second = f.authority.open(FILE, &f.root, &f.rel, "stale").unwrap();
    assert_eq!((second.epoch, second.revision), (first.epoch.clone(), 1));
    f.authority.close(FILE);
    let text = f.disk();
    let third = f.authority.open(FILE, &f.root, &f.rel, &text).unwrap();
    assert_ne!(third.epoch, first.epoch);
    assert_eq!(third.revision, 0);
}

#[test]
fn receipts_replay_the_original_ack_and_age_out() {
    let f = Fixture::new("authority-receipts");
    let epoch = f.open().epoch;
    let first = f.authority.commit(FILE, op("c", 1, add("b"))).unwrap();
    let replay = f.authority.commit(FILE, op("c", 1, add("b"))).unwrap();
    assert!(replay.replayed && replay.transaction.is_none());
    assert_eq!(
        (replay.revision, replay.epoch),
        (first.revision, epoch.clone())
    );
    assert_eq!(f.changes().len(), 1);
    let reused = Op {
        op_id: "other".into(),
        ..op("c", 1, add("z"))
    };
    assert_eq!(
        f.authority.commit(FILE, reused).unwrap_err().code,
        Code::Invalid
    );

    for seq in 2..=(receipts::WINDOW as u64 + 1) {
        f.authority
            .commit(FILE, op("c", seq, add(&format!("s{seq}"))))
            .unwrap();
    }
    assert_eq!(
        f.authority
            .commit(FILE, op("c", 1, add("b")))
            .unwrap_err()
            .code,
        Code::StaleOp
    );
    let last = receipts::WINDOW as u64 + 1;
    let replay = f
        .authority
        .commit(FILE, op("c", last, add(&format!("s{last}"))))
        .unwrap();
    assert_eq!((replay.replayed, replay.revision), (true, last));

    let foreign = Op {
        epoch: Some("not-this-one".into()),
        ..op("c", last + 1, add("late"))
    };
    assert_eq!(
        f.authority.commit(FILE, foreign).unwrap_err().code,
        Code::SessionEnded
    );
    let own = Op {
        epoch: Some(epoch),
        ..op("c", last + 1, add("late"))
    };
    assert!(f.authority.commit(FILE, own).is_ok());
}

#[test]
fn a_rejected_op_does_not_use_up_its_sequence_number() {
    let f = Fixture::new("authority-conflict");
    f.open();
    let stale =
        json!({"edits": [{"id": "a", "before": shape("a", 99.0), "after": shape("a", 5.0)}]});
    let error = f.authority.commit(FILE, op("c", 1, stale)).unwrap_err();
    assert_eq!(error.code, Code::ConflictShape);
    assert_eq!(f.disk(), f.text);
    let ack = f.authority.commit(FILE, op("c", 1, add("b"))).unwrap();
    assert_eq!((ack.revision, ack.replayed), (1, false));
}

#[test]
fn concurrent_clients_each_persist_exactly_once() {
    let f = Arc::new(Fixture::new("authority-threads"));
    f.open();
    let threads: Vec<_> = ["left", "right"]
        .into_iter()
        .map(|client| {
            let f = f.clone();
            std::thread::spawn(move || {
                for seq in 1..=20 {
                    f.authority
                        .commit(FILE, op(client, seq, add(&format!("{client}{seq}"))))
                        .unwrap();
                }
            })
        })
        .collect();
    for thread in threads {
        thread.join().unwrap();
    }
    let disk = f.disk();
    assert_eq!(ids(&disk).len(), 41);
    let revisions: Vec<u64> = f.changes().into_iter().map(|(r, _)| r).collect();
    assert_eq!(revisions, (1..=40).collect::<Vec<_>>());
    assert_eq!(f.rows.log().len(), 40);
    assert_eq!(f.authority.snapshot(FILE).unwrap().text, disk);
}

#[test]
fn an_external_edit_pauses_without_touching_the_file() {
    let f = Fixture::new("authority-external");
    f.open();
    f.authority.commit(FILE, op("c", 1, add("b"))).unwrap();
    let theirs = f.disk().replace("#ffffff", "#123456");
    std::fs::write(f.root.join(&f.rel), &theirs).unwrap();

    let error = f.authority.commit(FILE, op("c", 2, add("c"))).unwrap_err();
    assert_eq!(error.code, Code::ExternalEdit);
    assert_eq!(f.disk(), theirs);
    assert!(f.authority.snapshot(FILE).unwrap().paused);
    assert_eq!(
        f.authority
            .commit(FILE, op("c", 2, add("c")))
            .unwrap_err()
            .code,
        Code::ExternalEdit
    );
    assert_eq!(f.disk(), theirs);
    assert_eq!(f.phases(), [Phase::Paused]);

    let kept = f
        .authority
        .resolve_external(FILE, Resolution::KeepOurs)
        .unwrap();
    assert!(!kept.paused);
    assert_eq!(f.disk(), kept.text);
    assert!(kept.text.contains("#ffffff"));
    f.authority.commit(FILE, op("c", 2, add("c"))).unwrap();
    assert_eq!(ids(&f.disk()), ["a", "b", "c"]);

    std::fs::write(f.root.join(&f.rel), f.disk().replace("#ffffff", "dots")).unwrap();
    assert!(f.authority.commit(FILE, op("c", 3, add("d"))).is_err());
    let adopted = f
        .authority
        .resolve_external(FILE, Resolution::AdoptDisk)
        .unwrap();
    assert!(adopted.text.contains("\"background\": \"dots\""));
    assert_eq!(adopted.revision, 3);
    assert_eq!(f.changes().last(), Some(&(3, None)));
    f.authority.commit(FILE, op("c", 3, add("d"))).unwrap();
    assert!(f.disk().contains("\"background\": \"dots\""));
}

#[test]
fn delete_ends_the_session_and_nothing_recreates_the_file() {
    let f = Fixture::new("authority-delete");
    f.open();
    f.authority.delete(FILE).unwrap();
    let path = f.root.join(&f.rel);
    assert!(!path.exists());
    assert_eq!(
        f.authority
            .commit(FILE, op("c", 1, add("b")))
            .unwrap_err()
            .code,
        Code::SessionEnded
    );
    assert_eq!(
        f.authority.snapshot(FILE).unwrap_err().code,
        Code::SessionEnded
    );
    assert!(f.authority.open(FILE, &f.root, &f.rel, &f.text).is_err());
    assert!(!path.exists());
    assert_eq!(f.rows.log(), [format!("delete {FILE}")]);
    assert_eq!(f.phases(), [Phase::Ended]);
}

#[test]
fn a_file_removed_behind_the_session_ends_it() {
    let f = Fixture::new("authority-removed");
    f.open();
    std::fs::remove_file(f.root.join(&f.rel)).unwrap();
    let error = f.authority.commit(FILE, op("c", 1, add("b"))).unwrap_err();
    assert_eq!(error.code, Code::SessionEnded);
    assert!(!f.root.join(&f.rel).exists());
}

#[test]
fn rename_keeps_the_session() {
    let f = Fixture::new("authority-rename");
    let epoch = f.open().epoch;
    let moved = f.authority.rename(FILE, "Renamed").unwrap();
    assert_eq!(moved.relative_path, "courses/TEST/canvases/Renamed.canvas");
    let ack = f.authority.commit(FILE, op("c", 1, add("b"))).unwrap();
    assert_eq!(ack.epoch, epoch);
    assert!(!f.root.join(&f.rel).exists());
    let text = std::fs::read_to_string(f.root.join(&moved.relative_path)).unwrap();
    assert_eq!(ids(&text), ["a", "b"]);
    assert_eq!(
        f.authority.snapshot(FILE).unwrap().relative_path,
        moved.relative_path
    );
    assert_eq!(
        f.rows.log()[0],
        format!("rename {FILE} Renamed.canvas {}", moved.relative_path)
    );
}

#[test]
fn a_viewport_hint_is_saved_with_the_next_write_without_a_revision() {
    let f = Fixture::new("authority-viewport");
    f.open();
    let view = json!({"x": 40, "y": -10, "zoom": 2.5});
    let hint = Op {
        viewport: Some(view.clone()),
        ..op("desk", 1, json!({"edits": []}))
    };
    let ack = f.authority.commit(FILE, hint).unwrap();
    assert_eq!((ack.revision, ack.transaction), (0, None));
    assert_eq!(f.disk(), f.text);
    assert!(f.changes().is_empty());
    let bad = Op {
        viewport: Some(json!({"x": 0, "y": 0, "zoom": 0})),
        ..op("desk", 2, json!({"edits": []}))
    };
    assert_eq!(
        f.authority.commit(FILE, bad).unwrap_err().code,
        Code::Invalid
    );
    let viewport_tx = json!({"edits": [], "viewport": view});
    assert_eq!(
        f.authority
            .commit(FILE, op("desk", 2, viewport_tx))
            .unwrap_err()
            .code,
        Code::Invalid
    );

    f.authority.commit(FILE, op("ipad", 1, add("b"))).unwrap();
    let saved: Value = serde_json::from_str(&f.disk()).unwrap();
    assert_eq!(saved["viewport"], view);

    let later = json!({"x": 1, "y": 2, "zoom": 3});
    let hint = Op {
        viewport: Some(later.clone()),
        ..op("desk", 3, json!({"edits": []}))
    };
    f.authority.commit(FILE, hint).unwrap();
    f.authority.close(FILE);
    let saved: Value = serde_json::from_str(&f.disk()).unwrap();
    assert_eq!(saved["viewport"], later);
    assert_eq!(f.changes().len(), 1);
}

#[test]
fn a_large_embedded_image_round_trips() {
    use base64::Engine as _;
    let mut bytes = b"\x89PNG\r\n\x1a\n".to_vec();
    bytes.resize(15 * 1024 * 1024, 0x5a);
    let data_url = format!(
        "data:image/png;base64,{}",
        base64::engine::general_purpose::STANDARD.encode(&bytes)
    );
    assert!(data_url.len() > 20 * 1024 * 1024);
    let f = Fixture::new("authority-large");
    f.open();
    let mut image = shape("img", 0.0);
    image["type"] = json!("image");
    image["dataUrl"] = json!(data_url);
    image["naturalWidth"] = json!(4000);
    image["naturalHeight"] = json!(3000);
    let tx = json!({"edits": [{"id": "img", "before": null, "after": image}]});
    f.authority.commit(FILE, op("c", 1, tx)).unwrap();
    let moved = {
        let mut moved = image.clone();
        moved["x"] = json!(500.5);
        moved
    };
    let tx = json!({"edits": [{"id": "img", "before": image, "after": moved}]});
    f.authority.commit(FILE, op("c", 2, tx)).unwrap();
    let disk = f.disk();
    let value: Value = serde_json::from_str(&disk).unwrap();
    model::validate(&value).unwrap();
    assert_eq!(
        value["elements"][1]["dataUrl"].as_str(),
        Some(data_url.as_str())
    );
    assert_eq!(value["elements"][1]["x"], json!(500.5));
    assert_eq!(f.authority.snapshot(FILE).unwrap().text, disk);
}

#[test]
fn whole_file_mutations_cannot_bypass_a_live_or_paused_authority() {
    let f = Fixture::new("authority-legacy-fence");
    f.open();
    f.authority
        .commit(FILE, op("web:ipad", 1, add("remote")))
        .unwrap();
    let saved = f.disk();
    let legacy_write = || {
        f.authority.unshared(&f.root, &f.rel, || {
            super::super::super::write_at(&f.root, &f.rel, &f.text)
        })
    };
    assert!(legacy_write().is_err());
    assert_eq!(f.disk(), saved);
    assert!(!f.authority.snapshot(FILE).unwrap().paused);
    f.authority
        .commit(FILE, op("desktop", 1, add("local")))
        .unwrap();
    assert_eq!(ids(&f.disk()), ["a", "remote", "local"]);

    let external = f.disk().replace("#ffffff", "#123456");
    std::fs::write(f.root.join(&f.rel), &external).unwrap();
    assert!(f
        .authority
        .commit(FILE, op("desktop", 2, add("later")))
        .is_err());
    assert!(legacy_write().is_err());
    assert_eq!(f.disk(), external);
    f.authority.close(FILE);
    legacy_write().unwrap();
    assert_eq!(f.disk(), f.text);
}

#[test]
fn whole_file_fence_allows_other_canvases() {
    let f = Fixture::new("authority-other-file");
    f.open();
    let other = super::super::super::create_at(&f.root, "TEST", "Other", &f.text)
        .unwrap()
        .relative_path;
    f.authority
        .unshared(&f.root, &other, || {
            super::super::super::write_at(&f.root, &other, &f.text)
        })
        .unwrap();
    f.authority
        .commit(FILE, op("ipad", 1, add("remote")))
        .unwrap();
    assert_eq!(ids(&f.disk()), ["a", "remote"]);
}

#[test]
fn one_path_cannot_have_two_authority_writers() {
    let f = Fixture::new("authority-unique-path");
    f.open();
    let error = f
        .authority
        .open(FILE + 1, &f.root, &f.rel, &f.text)
        .unwrap_err();
    assert_eq!(error.code, Code::Invalid);
    f.authority.close(FILE);
    assert!(f.authority.open(FILE + 1, &f.root, &f.rel, &f.text).is_ok());
}
