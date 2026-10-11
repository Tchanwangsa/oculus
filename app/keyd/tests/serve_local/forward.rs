use std::io::{BufRead, BufReader, Write};
use std::time::{Duration, Instant};

use serde_json::{json, Value};

use crate::harness::{connect, Keyd, DEADLINE};

/// A one-shot HTTP origin that answers 429 with a Retry-After and records the
/// request it saw.
fn fake_origin() -> (String, std::thread::JoinHandle<String>) {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let origin = format!("http://127.0.0.1:{}", listener.local_addr().unwrap().port());
    listener.set_nonblocking(true).unwrap();
    let handle = std::thread::spawn(move || {
        let started = Instant::now();
        let stream = loop {
            match listener.accept() {
                Ok((stream, _)) => break stream,
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    assert!(
                        started.elapsed() < DEADLINE,
                        "keyd never reached the origin"
                    );
                    std::thread::sleep(Duration::from_millis(20));
                }
                Err(e) => panic!("origin accept: {e}"),
            }
        };
        // macOS hands the accepted socket the listener's non-blocking mode.
        stream.set_nonblocking(false).unwrap();
        stream.set_read_timeout(Some(DEADLINE)).unwrap();
        stream.set_write_timeout(Some(DEADLINE)).unwrap();
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
    let stream = connect(&keyd.sock);
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
    let stream = connect(&keyd.sock);
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

#[test]
fn the_binary_attaches_the_canvas_cookie_and_ed_token_and_streams_a_body() {
    if !cfg!(feature = "dev") {
        return;
    }
    let (canvas, canvas_seen) = fake_origin();
    let (ed, ed_seen) = fake_origin();
    let mut keyd = Keyd::start_with(
        60,
        &[
            ("OCULUS_KEYD_CANVAS_ORIGIN", &canvas),
            ("OCULUS_KEYD_ED_ORIGIN", &ed),
        ],
    );
    let put = |kind: &str, value: &str| {
        keyd.call(json!({"op": "session_put", "kind": kind, "value": value}))["stored"].clone()
    };
    assert_eq!(put("canvas", "canvas_session=SECRET-COOKIE"), true);
    assert_eq!(put("ed", "SECRET-TOKEN"), true);

    let (reply, answer) = forward(
        &keyd,
        json!({"op": "forward", "secret": "canvas", "method": "GET",
               "path": "/api/v1/courses?include[]=term"}),
        b"",
    );
    assert_eq!(reply["status"], 429, "{reply}");
    assert_eq!(answer, b"{\"detail\":\"slow down\"}");
    let head = canvas_seen.join().unwrap().to_lowercase();
    assert!(
        head.starts_with("get /api/v1/courses?include[]=term http/1.1"),
        "{head}"
    );
    assert!(
        head.contains("cookie: canvas_session=secret-cookie"),
        "{head}"
    );

    // Streamed: no body_len, and the body runs to the end of the connection.
    let stream = connect(&keyd.sock);
    let req = json!({"op": "forward", "secret": "ed", "method": "GET",
                     "path": "/api/threads/77?view=1", "stream": true});
    (&stream).write_all(format!("{req}\n").as_bytes()).unwrap();
    let mut reader = BufReader::new(&stream);
    let mut line = String::new();
    reader.read_line(&mut line).unwrap();
    let reply: Value = serde_json::from_str(&line).unwrap();
    assert_eq!(reply["status"], 429, "{reply}");
    assert!(reply.get("body_len").is_none(), "{reply}");
    let mut body = Vec::new();
    std::io::Read::read_to_end(&mut reader, &mut body).unwrap();
    assert_eq!(body, b"{\"detail\":\"slow down\"}");
    let head = ed_seen.join().unwrap().to_lowercase();
    assert!(head.contains("x-token: secret-token"), "{head}");
    assert!(!head.contains("cookie:"), "{head}");

    drop(stream);
    keyd.child.kill().ok();
    keyd.child.wait().ok();
    let mut log = String::new();
    std::io::Read::read_to_string(keyd.child.stderr.as_mut().unwrap(), &mut log).unwrap();
    assert!(
        log.contains("secret=canvas") && log.contains("secret=ed") && log.contains("streamed"),
        "{log}"
    );
    for leak in ["SECRET", "include[]", "threads/77", "slow down"] {
        assert!(!log.contains(leak), "{leak} in {log}");
    }
}
