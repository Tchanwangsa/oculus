use super::client::parse_next_link;
use super::cookies::merged_cookie_header;
use super::{Canvas, CANCELLED};
use std::io::{Read, Write};
use std::path::PathBuf;
use std::sync::Mutex;

fn sc(vals: &[&str]) -> Vec<String> {
    vals.iter().map(|s| s.to_string()).collect()
}

#[test]
fn replaces_rotated_value_in_place() {
    let out = merged_cookie_header(
        "a=1; canvas_session=OLD; z=9",
        &sc(&["canvas_session=NEW; path=/; secure; httponly"]),
    );
    // Position must be preserved, not appended to the end.
    assert_eq!(out.unwrap(), "a=1; canvas_session=NEW; z=9");
}

#[test]
fn appends_cookies_not_seen_before() {
    assert_eq!(
        merged_cookie_header("a=1", &sc(&["b=2; path=/"])).unwrap(),
        "a=1; b=2"
    );
}

#[test]
fn no_write_when_value_is_unchanged() {
    assert!(merged_cookie_header("a=1; b=2", &sc(&["b=2; path=/"])).is_none());
}

#[test]
fn ignores_junk_and_empty_store() {
    assert!(merged_cookie_header("", &sc(&["a=1"])).is_none());
    assert!(merged_cookie_header("a=1", &sc(&["novalue; path=/"])).is_none());
}

#[test]
fn keeps_base64_padding_in_values() {
    // Session values are base64 ending in '='; split on the first '=' only.
    let out = merged_cookie_header("s=old", &sc(&["s=abc==; path=/"])).unwrap();
    assert_eq!(out, "s=abc==");
}

/// Serves one canned response on a loopback port; returns its URL.
fn serve_once(response: Vec<u8>) -> String {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        let mut head = [0u8; 4096];
        let _ = stream.read(&mut head);
        let _ = stream.write_all(&response);
    });
    format!("http://{addr}/video.mp4")
}

fn signed_out() -> Canvas {
    Canvas {
        cookie: Mutex::new(String::new()),
        cookie_path: PathBuf::new(),
    }
}

#[test]
fn a_download_streams_to_disk_and_refuses_a_short_body() {
    let dir = crate::test_support::Scratch::new("canvas-download");
    let body = vec![7u8; 300_000];
    let mut whole = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: video/mp4\r\nContent-Length: {}\r\n\r\n",
        body.len()
    )
    .into_bytes();
    whole.extend_from_slice(&body);
    let seen = Mutex::new(Vec::new());
    let dest = dir.join("whole.mp4");
    let n = signed_out()
        .download_to(
            &serve_once(whole),
            &dest,
            &|p| seen.lock().unwrap().push(p),
            &|| false,
        )
        .unwrap();
    assert_eq!(n, 300_000);
    assert_eq!(std::fs::read(&dest).unwrap(), body);
    assert_eq!(seen.lock().unwrap().last(), Some(&100));

    let short = b"HTTP/1.1 200 OK\r\nContent-Length: 100\r\n\r\nonly this".to_vec();
    let err = signed_out()
        .download_to(&serve_once(short), &dir.join("short.mp4"), &|_| {}, &|| {
            false
        })
        .unwrap_err();
    assert_ne!(err, CANCELLED);
}

#[test]
fn a_cancelled_download_says_so() {
    let dir = crate::test_support::Scratch::new("canvas-cancel");
    let ok = b"HTTP/1.1 200 OK\r\nContent-Length: 4\r\n\r\nmp4!".to_vec();
    let err = signed_out()
        .download_to(&serve_once(ok), &dir.join("x.mp4"), &|_| {}, &|| true)
        .unwrap_err();
    assert_eq!(err, CANCELLED);
}

#[test]
fn finds_next_page_in_link_header() {
    let link = r#"<https://c/api?page=1>; rel="current", <https://c/api?page=2>; rel="next""#;
    assert_eq!(parse_next_link(link).unwrap(), "https://c/api?page=2");
    assert!(parse_next_link(r#"<https://c/api?page=9>; rel="last""#).is_none());
}
