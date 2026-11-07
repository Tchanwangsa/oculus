use std::io::Write;
use std::process::{Command, Stdio};

use serde_json::{json, Value};

use super::replay::Replay;

pub(super) fn project(replay: &Replay) -> Value {
    let app = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap();
    let mut child = Command::new("bun")
        .arg("run")
        .arg("tests/harness/project-replay.ts")
        .current_dir(app)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("Bun is required for the production row projection");
    child.stdin.take().unwrap().write_all(serde_json::to_vec(&json!({
        "envelopes": replay.envelopes, "reopened": replay.reopened, "runs": replay.ledger["runs"], "repeatEnvelope": replay.repeat_envelope, "loadRace": replay.load_race, "snapshots": replay.snapshots,
    })).unwrap().as_slice()).unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(
        output.status.success(),
        "row projection: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).unwrap()
}
