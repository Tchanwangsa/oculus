//! The contract check between this server and the browser client: the real
//! `ShareClient` (`app/tests/e2e/shareClientE2E.ts`, run by bun) against a
//! real server, authority and canvas file, with this test playing the
//! desktop. Ignored by default because it needs bun:
//! `cargo test --lib companion::tests::e2e -- --ignored --nocapture`.
//! See docs/device-sharing.md "End-to-end check".

use super::{document, shape, Harness, FILE};
use crate::companion::limits::Limits;
use crate::library::files::canvases::authority::events::Event;
use crate::library::files::canvases::authority::model;
use crate::library::files::canvases::authority::session::Op;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::io::{BufRead, BufReader, Read, Write};
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// The whole script, including a 10 MiB image both ways, runs in seconds;
/// this only bounds a hang.
const DEADLINE: Duration = Duration::from_secs(300);

/// One accepted change, as the authority announced it.
struct Seen {
    epoch: String,
    revision: u64,
    client: Option<String>,
    /// Ids the change added (`before: null`).
    added: Vec<String>,
}

fn added_ids(transaction: &Value) -> Vec<String> {
    transaction["edits"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|edit| edit["before"].is_null() && !edit["after"].is_null())
        .filter_map(|edit| edit["id"].as_str().map(str::to_string))
        .collect()
}

#[test]
#[ignore = "needs bun; run with --ignored"]
fn the_browser_client_and_the_server_agree() {
    // Pings every 200 ms, so the page's text-ping liveness is exercised.
    let limits = Limits {
        ping_every: Duration::from_millis(200),
        dead_after: Duration::from_secs(1),
        ..Limits::default()
    };
    let h = Harness::with("e2e", limits, 0);
    let rel = h.open(FILE, &document(vec![shape("base", 0.0)]));
    // A second shared canvas keeps the server on its port while the first
    // share stops and restarts, as when another canvas stays shared.
    h.open(FILE + 1, &document(vec![]));
    let keeper = h.share(FILE + 1);
    let share = h.share(FILE);
    assert_eq!(share.port, keeper.port);

    let seen: Arc<Mutex<Vec<Seen>>> = Arc::default();
    let sink = seen.clone();
    let desktop_events: Arc<Mutex<Vec<Value>>> = Arc::default();
    let desktop_sink = desktop_events.clone();
    h.authority.subscribers().subscribe(move |event| {
        if let Event::Change(change) = event {
            if change.file_id == FILE {
                desktop_sink.lock().unwrap().push(json!({
                    "kind": "change", "fileId": change.file_id, "epoch": change.epoch,
                    "revision": change.revision, "clientId": change.client_id,
                    "transaction": change.transaction.as_ref(),
                }));
                sink.lock().unwrap().push(Seen {
                    epoch: change.epoch.clone(),
                    revision: change.revision,
                    client: change.client_id.clone(),
                    added: added_ids(&change.transaction),
                });
            }
        }
    });

    let app = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
    let mut child = Command::new("bun")
        .arg("tests/e2e/shareClientE2E.ts")
        .current_dir(&app)
        .env("OCULUS_E2E_PORT", share.port.to_string())
        .env("OCULUS_E2E_KEY", &share.token)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("bun on PATH");
    let mut stdin = child.stdin.take().unwrap();
    let stdout = BufReader::new(child.stdout.take().unwrap());
    let mut stderr = child.stderr.take().unwrap();
    let errors = std::thread::spawn(move || {
        let mut text = String::new();
        stderr.read_to_string(&mut text).ok();
        text
    });
    let pid = child.id();
    let (finished, deadline) = mpsc::channel::<()>();
    let watchdog = std::thread::spawn(move || {
        if deadline.recv_timeout(DEADLINE).is_err() {
            Command::new("kill").arg(pid.to_string()).status().ok();
        }
    });

    let mut output = String::new();
    let mut image: Option<Value> = None;
    let mut first_epoch: Option<(String, u64)> = None;
    let mut secrets = vec![share.secret().to_string(), keeper.secret().to_string()];
    let mut done = false;
    for line in stdout.lines() {
        let Ok(line) = line else { break };
        output.push_str(&line);
        output.push('\n');
        let Some(request) = line.strip_prefix("@@ ") else {
            continue;
        };
        let (command, args) = request.split_once(' ').unwrap_or((request, "{}"));
        let args: Value = serde_json::from_str(args).unwrap_or(Value::Null);
        let answer: Result<Value, String> = match command {
            "desktop-snapshot" => h
                .authority
                .snapshot(FILE)
                .map(|snapshot| serde_json::to_value(snapshot).unwrap())
                .map_err(|error| error.to_string()),
            "desktop-events" => Ok(json!(std::mem::take(&mut *desktop_events.lock().unwrap()))),
            "desktop-op" => h
                .authority
                .commit(
                    FILE,
                    Op {
                        client_id: args["clientId"].as_str().unwrap().into(),
                        seq: args["seq"].as_u64().unwrap(),
                        op_id: args["opId"].as_str().unwrap().into(),
                        transaction: args["transaction"].clone(),
                        viewport: args.get("viewport").cloned(),
                        epoch: args["epoch"].as_str().map(str::to_string),
                    },
                )
                .map(|ack| serde_json::to_value(ack).unwrap())
                .map_err(|error| error.to_string()),
            "desktop-commit" => {
                let id = args["id"].as_str().unwrap_or("d");
                h.authority
                    .commit(
                        FILE,
                        Op {
                            client_id: "desk".into(),
                            seq: args["seq"].as_u64().unwrap_or(1),
                            op_id: format!("desk-{id}"),
                            transaction: json!({"edits": [{"id": id, "before": null, "after": shape(id, 50.0)}]}),
                            viewport: None,
                            epoch: None,
                        },
                    )
                    .map(|ack| json!({"revision": ack.revision}))
                    .map_err(|error| error.to_string())
            }
            "expect-image" => {
                image = Some(args);
                Ok(json!({}))
            }
            "stop-share" => {
                // What the desktop's Stop does: stop the share, then detach
                // the authority, which ends the epoch.
                let snapshot = h.authority.snapshot(FILE).unwrap();
                first_epoch = Some((snapshot.epoch, snapshot.revision));
                h.companion.share_stop(FILE);
                h.authority.close(FILE);
                Ok(json!({}))
            }
            "restart-share" => {
                let text = std::fs::read_to_string(h.root.join(&rel)).unwrap();
                h.authority.open(FILE, &h.root, &rel, &text).unwrap();
                let again = h.share(FILE);
                assert_eq!(again.port, share.port, "the server kept its port");
                secrets.push(again.secret().to_string());
                Ok(json!({"key": again.token}))
            }
            "done" => {
                done = true;
                Ok(json!({}))
            }
            other => Err(format!("unknown command {other}")),
        };
        let reply = match answer {
            Ok(value) => format!("@@ok {value}\n"),
            Err(message) => format!("@@err {message}\n"),
        };
        if stdin
            .write_all(reply.as_bytes())
            .and_then(|_| stdin.flush())
            .is_err()
        {
            break;
        }
    }
    drop(stdin);
    let status = child.wait().unwrap();
    finished.send(()).ok();
    watchdog.join().ok();
    let errors = errors.join().unwrap();
    eprint!("{errors}");
    assert!(
        status.success() && done,
        "the client script failed ({status}):\n{errors}"
    );

    // Nothing the page wrote, on either stream, carries a key.
    for secret in &secrets {
        assert!(
            !output.contains(secret.as_str()) && !errors.contains(secret.as_str()),
            "a share secret reached the script's output"
        );
    }

    // The file on disk is a valid canvas with each stroke exactly once.
    let text = std::fs::read_to_string(h.root.join(&rel)).unwrap();
    let saved: Value = serde_json::from_str(&text).unwrap();
    model::validate(&saved).unwrap();
    let elements = saved["elements"].as_array().unwrap();
    let mut counts = BTreeMap::<&str, usize>::new();
    for element in elements {
        *counts.entry(element["id"].as_str().unwrap()).or_default() += 1;
    }
    let expected = [
        "base",
        "a1",
        "b1",
        "d1",
        "rejoined-web",
        "rejoined-desk",
        "img",
        "e1",
        "f1",
        "h1",
    ];
    assert_eq!(
        counts.keys().copied().collect::<Vec<_>>(),
        {
            let mut sorted = expected.to_vec();
            sorted.sort();
            sorted
        },
        "the saved canvas's strokes"
    );
    assert!(counts.values().all(|&n| n == 1), "a duplicate: {counts:?}");
    let x = |id: &str| elements.iter().find(|e| e["id"] == id).unwrap()["x"].clone();
    assert_eq!((x("a1"), x("b1")), (json!(100), json!(200)));
    assert!(x("base") == json!(11) || x("base") == json!(22));

    // The image arrived byte for byte.
    let image = image.expect("the script described its image");
    let saved_image = elements.iter().find(|e| e["id"] == image["id"]).unwrap();
    let data_url = saved_image["dataUrl"].as_str().unwrap();
    assert_eq!(data_url.len() as u64, image["length"].as_u64().unwrap());
    let digest: String = Sha256::digest(data_url.as_bytes())
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    assert_eq!(digest, image["sha256"].as_str().unwrap());

    // The first epoch includes image deletion and restoration; every revision is
    // ordered, each stroke is added once, and the next epoch has fresh clients.
    let (first, first_head) = first_epoch.expect("the share was stopped");
    let now = h.authority.snapshot(FILE).unwrap();
    assert_ne!(now.epoch, first);
    assert_eq!((first_head, now.revision), (13, 1));
    let seen = seen.lock().unwrap();
    let revisions = |epoch: &str| -> Vec<u64> {
        seen.iter()
            .filter(|s| s.epoch == epoch)
            .map(|s| s.revision)
            .collect()
    };
    assert_eq!(revisions(&first), (1..=13).collect::<Vec<_>>());
    assert_eq!(revisions(&now.epoch), vec![1]);
    let mut additions = BTreeMap::<&str, usize>::new();
    for id in seen.iter().flat_map(|s| s.added.iter()) {
        *additions.entry(id.as_str()).or_default() += 1;
    }
    assert!(
        additions
            .iter()
            .all(|(&id, &n)| n == if id == "img" { 2 } else { 1 })
            && !additions.contains_key("g1"),
        "{additions:?}"
    );
    for change in seen.iter() {
        let client = change.client.as_deref().unwrap_or("");
        if change
            .added
            .iter()
            .any(|id| id == "d1" || id == "rejoined-desk")
        {
            assert!(!client.starts_with("web:"));
        } else {
            assert!(client.starts_with("web:"), "{client}");
        }
    }
    let first_clients: Vec<&str> = seen
        .iter()
        .filter(|s| s.epoch == first)
        .filter_map(|s| s.client.as_deref())
        .collect();
    assert!(seen
        .iter()
        .filter(|s| s.epoch == now.epoch)
        .all(|s| !first_clients.contains(&s.client.as_deref().unwrap_or(""))));
}
