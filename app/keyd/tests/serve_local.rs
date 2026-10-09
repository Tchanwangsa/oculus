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
        Keyd::start_with(idle_secs, &[])
    }

    /// `origins` sets each `(variable, origin)`, e.g. a fake Voyage under
    /// `OCULUS_KEYD_VOYAGE_ORIGIN`.
    fn start_with(idle_secs: u64, origins: &[(&str, &str)]) -> Keyd {
        static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let n = NEXT.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        let dir = std::env::temp_dir().join(format!("keyd-bin-{}-{n}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let sock = dir.join("k.sock");
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_oculus-keyd"));
        cmd.arg("serve-local")
            .arg(&sock)
            .env("OCULUS_KEYD_DATA_DIR", &dir)
            .env("OCULUS_KEYD_TEST_KEY", "11".repeat(32))
            .env("OCULUS_KEYD_IDLE_SECS", idle_secs.to_string())
            .stderr(Stdio::piped());
        for (var, origin) in origins {
            cmd.env(var, origin);
        }
        let child = cmd.spawn().unwrap();
        let started = Instant::now();
        while !sock.exists() {
            assert!(
                started.elapsed() < Duration::from_secs(10),
                "keyd never bound its socket"
            );
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

/// A one-shot HTTP origin that answers 429 with a Retry-After and records the
/// request it saw.
fn fake_origin() -> (String, std::thread::JoinHandle<String>) {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let origin = format!("http://127.0.0.1:{}", listener.local_addr().unwrap().port());
    let handle = std::thread::spawn(move || {
        let (stream, _) = listener.accept().unwrap();
        let mut reader = BufReader::new(&stream);
        let mut head = String::new();
        loop {
            let mut line = String::new();
            reader.read_line(&mut line).unwrap();
            if line == "\r\n" {
                break;
            }
            head.push_str(&line);
        }
        let len: usize = head
            .lines()
            .find_map(|l| {
                l.to_lowercase()
                    .strip_prefix("content-length:")
                    .map(|v| v.trim().parse().unwrap())
            })
            .unwrap_or(0);
        let mut body = vec![0; len];
        std::io::Read::read_exact(&mut reader, &mut body).unwrap();
        let answer = b"{\"detail\":\"slow down\"}";
        let mut w = &stream;
        write!(
            w,
            "HTTP/1.1 429 X\r\nRetry-After: 9\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
            answer.len()
        )
        .unwrap();
        w.write_all(answer).unwrap();
        head
    });
    (origin, handle)
}

#[test]
fn the_binary_forwards_to_its_test_origin_and_never_logs_the_key() {
    if !cfg!(feature = "dev") {
        return;
    }
    let (origin, seen) = fake_origin();
    let mut keyd = Keyd::start_with(60, &[("OCULUS_KEYD_VOYAGE_ORIGIN", &origin)]);
    assert_eq!(
        keyd.call(json!({"op": "store", "secret": "voyage", "value": "pa-SECRET"}))["stored"],
        true
    );

    let body = b"{\"inputs\":[\"BODY-TEXT\"]}";
    let stream = UnixStream::connect(&keyd.sock).unwrap();
    let req = json!({"op": "forward", "secret": "voyage", "method": "POST", "path": "/v1/multimodalembeddings",
                     "headers": [["Content-Type", "application/json"]], "body_len": body.len()});
    (&stream).write_all(format!("{req}\n").as_bytes()).unwrap();
    (&stream).write_all(body).unwrap();
    let mut reader = BufReader::new(&stream);
    let mut line = String::new();
    reader.read_line(&mut line).unwrap();
    let reply: Value = serde_json::from_str(&line).unwrap();
    assert_eq!(reply["status"], 429, "{reply}");
    assert!(
        reply["headers"]
            .as_array()
            .unwrap()
            .iter()
            .any(|h| h[0] == "retry-after" && h[1] == "9"),
        "{reply}"
    );
    let mut answer = vec![0; reply["body_len"].as_u64().unwrap() as usize];
    std::io::Read::read_exact(&mut reader, &mut answer).unwrap();
    assert_eq!(answer, b"{\"detail\":\"slow down\"}");
    assert!(!line.contains("pa-SECRET"));

    let head = seen.join().unwrap();
    assert!(
        head.starts_with("POST /v1/multimodalembeddings HTTP/1.1"),
        "{head}"
    );
    assert!(
        head.to_lowercase()
            .contains("authorization: bearer pa-secret"),
        "{head}"
    );

    drop(stream);
    keyd.child.kill().ok();
    keyd.child.wait().ok();
    let mut log = String::new();
    std::io::Read::read_to_string(keyd.child.stderr.as_mut().unwrap(), &mut log).unwrap();
    assert!(
        log.contains("op=forward") && log.contains("status=429"),
        "{log}"
    );
    for leak in ["pa-SECRET", "BODY-TEXT", "slow down", "application/json"] {
        assert!(!log.contains(leak), "{leak} in {log}");
    }
}

/// One `forward` and its reply: header, then body.
fn forward(keyd: &Keyd, req: Value, body: &[u8]) -> (Value, Vec<u8>) {
    let stream = UnixStream::connect(&keyd.sock).unwrap();
    (&stream).write_all(format!("{req}\n").as_bytes()).unwrap();
    (&stream).write_all(body).unwrap();
    let mut reader = BufReader::new(&stream);
    let mut line = String::new();
    reader.read_line(&mut line).unwrap();
    let reply: Value = serde_json::from_str(&line).unwrap();
    let mut answer = vec![0; reply["body_len"].as_u64().unwrap_or(0) as usize];
    std::io::Read::read_exact(&mut reader, &mut answer).unwrap();
    (reply, answer)
}

#[test]
fn mineru_and_groq_forward_to_their_own_test_origins() {
    if !cfg!(feature = "dev") {
        return;
    }
    let (mineru, mineru_seen) = fake_origin();
    let (groq, groq_seen) = fake_origin();
    let keyd = Keyd::start_with(
        60,
        &[
            ("OCULUS_KEYD_MINERU_ORIGIN", &mineru),
            ("OCULUS_KEYD_GROQ_ORIGIN", &groq),
        ],
    );
    for (secret, value) in [("mineru", "mineru-SECRET"), ("groq", "gsk_SECRET")] {
        assert_eq!(
            keyd.call(json!({"op": "store", "secret": secret, "value": value}))["stored"],
            true
        );
    }

    let (reply, _) = forward(
        &keyd,
        json!({"op": "forward", "secret": "mineru", "method": "GET",
               "path": "/api/v4/extract-results/batch/b-1", "headers": [["Accept", "application/json"]]}),
        b"",
    );
    assert_eq!(reply["status"], 429, "{reply}");
    let head = mineru_seen.join().unwrap();
    assert!(
        head.starts_with("GET /api/v4/extract-results/batch/b-1 HTTP/1.1"),
        "{head}"
    );
    assert!(
        head.to_lowercase()
            .contains("authorization: bearer mineru-secret"),
        "{head}"
    );

    let body = b"--B\r\nContent-Disposition: form-data; name=\"model\"\r\n\r\nx\r\n--B--\r\n";
    let (reply, _) = forward(
        &keyd,
        json!({"op": "forward", "secret": "groq", "method": "POST",
               "path": "/openai/v1/audio/transcriptions",
               "headers": [["Content-Type", "multipart/form-data; boundary=B"]], "body_len": body.len()}),
        body,
    );
    assert_eq!(reply["status"], 429, "{reply}");
    let head = groq_seen.join().unwrap();
    assert!(
        head.starts_with("POST /openai/v1/audio/transcriptions HTTP/1.1"),
        "{head}"
    );
    let lower = head.to_lowercase();
    assert!(lower.contains("authorization: bearer gsk_secret"), "{head}");
    assert!(
        lower.contains("content-type: multipart/form-data; boundary=b"),
        "{head}"
    );
}
