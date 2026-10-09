//! The built binary end to end through its debug-only `serve-local` hook:
//! a private socket, data dir and master key, never launchd or the keychain.
//! Without the `dev` feature this test process is refused, because it is not
//! inside keyd's app bundle; with it, the ops run.

use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use serde_json::{json, Value};

struct Keyd {
    child: Child,
    dir: PathBuf,
    sock: PathBuf,
}

impl Keyd {
    fn start(idle_secs: u64) -> Keyd {
        static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let n = NEXT.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        let dir = std::env::temp_dir().join(format!("keyd-bin-{}-{n}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let sock = dir.join("k.sock");
        let child = Command::new(env!("CARGO_BIN_EXE_oculus-keyd"))
            .arg("serve-local")
            .arg(&sock)
            .env("OCULUS_KEYD_DATA_DIR", &dir)
            .env("OCULUS_KEYD_TEST_KEY", "11".repeat(32))
            .env("OCULUS_KEYD_IDLE_SECS", idle_secs.to_string())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        let started = Instant::now();
        while !sock.exists() {
            assert!(started.elapsed() < Duration::from_secs(10), "keyd never bound its socket");
            std::thread::sleep(Duration::from_millis(20));
        }
        Keyd { child, dir, sock }
    }

    fn call(&self, req: Value) -> Value {
        let stream = UnixStream::connect(&self.sock).unwrap();
        (&stream).write_all(format!("{req}\n").as_bytes()).unwrap();
        let mut line = String::new();
        BufReader::new(&stream).read_line(&mut line).unwrap();
        serde_json::from_str(&line).unwrap()
    }
}

impl Drop for Keyd {
    fn drop(&mut self) {
        self.child.kill().ok();
        self.child.wait().ok();
        std::fs::remove_dir_all(&self.dir).ok();
    }
}

#[test]
fn the_binary_serves_or_refuses_by_its_build() {
    let keyd = Keyd::start(60);
    let ping = keyd.call(json!({"op": "ping"}));
    if !cfg!(feature = "dev") {
        assert_eq!(ping["error"], "caller", "{ping}");
        return;
    }
    assert_eq!(ping["source_hash"].as_str().unwrap().len(), 64);
    let out = Command::new(env!("CARGO_BIN_EXE_oculus-keyd")).arg("source-hash").output().unwrap();
    assert_eq!(String::from_utf8(out.stdout).unwrap().trim(), ping["source_hash"]);

    assert_eq!(keyd.call(json!({"op": "store", "secret": "voyage", "value": "pa-SECRET"}))["stored"], true);
    assert_eq!(keyd.call(json!({"op": "has", "secret": "voyage"}))["has"], true);
    let sealed = std::fs::read(keyd.dir.join("vault.bin")).unwrap();
    assert!(!sealed.windows(9).any(|w| w == b"pa-SECRET"), "vault.bin is ciphertext");
    assert_eq!(keyd.call(json!({"op": "delete", "secret": "voyage"}))["existed"], true);
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
        assert!(started.elapsed() < Duration::from_secs(10), "keyd did not exit when idle");
        std::thread::sleep(Duration::from_millis(50));
    };
    assert!(status.success());
    let mut log = String::new();
    std::io::Read::read_to_string(keyd.child.stderr.as_mut().unwrap(), &mut log).unwrap();
    assert!(log.contains("op=store"), "{log}");
    assert!(log.contains("idle for 1s, exiting"), "{log}");
    assert!(!log.contains("gsk-SECRET"));
}
