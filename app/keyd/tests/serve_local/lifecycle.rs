use std::process::Command;
use std::time::{Duration, Instant};

use serde_json::json;

use crate::harness::Keyd;

#[test]
fn the_binary_serves_or_refuses_by_its_build() {
    let keyd = Keyd::start(60);
    let ping = keyd.call(json!({"op": "ping"}));
    if !cfg!(feature = "dev") {
        assert_eq!(ping["error"], "caller", "{ping}");
        return;
    }
    assert_eq!(ping["source_hash"].as_str().unwrap().len(), 64);
    let out = Command::new(env!("CARGO_BIN_EXE_oculus-keyd"))
        .arg("source-hash")
        .output()
        .unwrap();
    assert_eq!(
        String::from_utf8(out.stdout).unwrap().trim(),
        ping["source_hash"]
    );

    assert_eq!(
        keyd.call(json!({"op": "store", "secret": "voyage", "value": "pa-SECRET"}))["stored"],
        true
    );
    assert_eq!(
        keyd.call(json!({"op": "has", "secret": "voyage"}))["has"],
        true
    );
    let sealed = std::fs::read(keyd.dir.join("vault.bin")).unwrap();
    assert!(
        !sealed.windows(9).any(|w| w == b"pa-SECRET"),
        "vault.bin is ciphertext"
    );
    assert_eq!(
        keyd.call(json!({"op": "delete", "secret": "voyage"}))["existed"],
        true
    );
}

#[test]
fn the_binary_exits_when_idle_and_never_logs_a_value() {
    let mut keyd = Keyd::start(1);
    keyd.call(json!({"op": "store", "secret": "groq", "value": "gsk-SECRET"}));
    let started = Instant::now();
    let status = loop {
        if let Some(status) = keyd.child.try_wait().unwrap() {
            break status;
        }
        assert!(
            started.elapsed() < Duration::from_secs(10),
            "keyd did not exit when idle"
        );
        std::thread::sleep(Duration::from_millis(50));
    };
    assert!(status.success());
    let mut log = String::new();
    std::io::Read::read_to_string(keyd.child.stderr.as_mut().unwrap(), &mut log).unwrap();
    assert!(log.contains("op=store"), "{log}");
    assert!(log.contains("idle for 1s, exiting"), "{log}");
    assert!(!log.contains("gsk-SECRET"));
}
