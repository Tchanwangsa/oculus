use super::client::{get, joined, next_json, next_of, open_response, post, socket};
use super::{add, document, image_document, Harness, FILE};
use crate::companion::error::Code;
use crate::companion::grants::EndReason;
use crate::companion::limits::Limits;
use serde_json::json;
use std::time::{Duration, Instant};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

async fn drain(stream: &mut TcpStream) -> usize {
    let mut total = 0;
    let mut buf = vec![0u8; 256 * 1024];
    loop {
        match tokio::time::timeout(Duration::from_secs(5), stream.read(&mut buf)).await {
            Ok(Ok(0)) | Ok(Err(_)) => return total,
            Ok(Ok(n)) => total += n,
            Err(_) => panic!("the response neither finished nor closed"),
        }
    }
}

#[tokio::test]
async fn stopping_a_share_ends_its_sockets_and_requests_only() {
    let h = Harness::new("revoke");
    h.open(FILE, &image_document(40 * 1024 * 1024));
    h.open(FILE + 1, &document(vec![]));
    let s = h.share(FILE);
    let t = h.share(FILE + 1);
    assert_eq!(s.port, t.port);
    assert_ne!(s.secret(), t.secret());
    let (mut socket, _) = joined(s.port, s.id(), s.secret(), "one").await;
    let (mut other, _) = joined(t.port, t.id(), t.secret(), "two").await;
    let (mut stream, head) = open_response(
        s.port,
        &s.path("snapshot"),
        &[("Authorization", &s.bearer())],
    )
    .await;
    assert_eq!(head.status, 200);
    let length: usize = head.header("content-length").unwrap().parse().unwrap();
    let started = Instant::now();
    h.companion.share_stop(FILE);
    let ended = next_of(&mut socket, "session-ended").await.unwrap();
    assert_eq!(ended["reason"], "stopped");
    assert_eq!(next_json(&mut socket).await, None);
    let closed = started.elapsed();
    let read = head.body.len() + drain(&mut stream).await;
    let aborted = started.elapsed();
    assert!(
        read < length,
        "the snapshot stopped mid-way ({read} of {length})"
    );
    eprintln!("[companion] revoke: socket closed in {closed:?}, snapshot cut in {aborted:?}");
    assert!(closed < Duration::from_secs(1), "{closed:?}");
    let gone = get(
        s.port,
        &s.path("snapshot"),
        &[("Authorization", &s.bearer())],
    )
    .await;
    assert_eq!(
        (gone.status, gone.json()["code"].clone()),
        (410, json!("session-ended"))
    );
    let commit = json!({"clientId": "two", "seq": 1, "opId": "two-1", "epoch": t.info.epoch, "transaction": add("b")});
    assert_eq!(
        post(t.port, &t.path("commit"), &t.bearer(), &commit)
            .await
            .status,
        200
    );
    assert_eq!(next_of(&mut other, "change").await.unwrap()["revision"], 1);
    assert_eq!(h.companion.shares().len(), 1);
}

#[tokio::test]
async fn shutdown_is_quick_whatever_the_clients_do() {
    let h = Harness::new("shutdown");
    h.open(FILE, &document(vec![]));
    let s = h.share(FILE);
    let mut sockets = Vec::new();
    for n in 0..3 {
        sockets.push(joined(s.port, s.id(), s.secret(), &format!("c{n}")).await.0);
    }
    let _waiting = socket(s.port, s.id()).await.unwrap();
    let _idle = TcpStream::connect(("127.0.0.1", s.port)).await.unwrap();
    let mut partial = TcpStream::connect(("127.0.0.1", s.port)).await.unwrap();
    partial.write_all(b"GET /c/ HTTP/1.1\r\nHo").await.unwrap();
    tokio::time::sleep(Duration::from_millis(50)).await;
    let started = Instant::now();
    h.companion.stop_all(EndReason::Shutdown);
    let took = started.elapsed();
    eprintln!(
        "[companion] shutdown with 3 sockets, 1 waiting socket, 2 idle connections: {took:?}"
    );
    assert!(took < Duration::from_millis(1100), "{took:?}");
    for socket in &mut sockets {
        let ended = next_of(socket, "session-ended").await.unwrap();
        assert_eq!(ended["reason"], "shutdown");
    }
    assert!(TcpStream::connect(("127.0.0.1", s.port)).await.is_err());
    let state = h.companion.state();
    assert!(!state.running && state.shares.is_empty());
    assert!(!h.states.lock().unwrap().last().unwrap().running);
}

#[tokio::test]
async fn a_taken_preferred_port_falls_through_to_the_next() {
    for blocker in ["127.0.0.1:0", "0.0.0.0:0"] {
        let blocker = std::net::TcpListener::bind(blocker).unwrap();
        let taken = blocker.local_addr().unwrap().port();
        let h = Harness::with("fallback", Limits::default(), taken);
        h.open(FILE, &document(vec![]));
        let s = h.share(FILE);
        assert!(
            s.port > taken && s.port < taken.saturating_add(20),
            "{taken} -> {}",
            s.port
        );
        let state = h.companion.state();
        eprintln!(
            "[companion] preferred {taken} taken by {:?}; bound {} on IPv4{}",
            blocker.local_addr().unwrap().ip(),
            s.port,
            if state.ipv6 { " and IPv6" } else { " only" }
        );
        if state.ipv6 {
            assert!(TcpStream::connect(("::1", s.port)).await.is_ok());
        }
        h.companion.stop_all(EndReason::Stopped);
    }
}

#[tokio::test]
async fn ending_the_authority_session_revokes_the_share() {
    let h = Harness::new("authority-ended");
    h.open(FILE, &document(vec![]));
    let s = h.share(FILE);
    let (mut socket, _) = joined(s.port, s.id(), s.secret(), "one").await;
    h.authority.close(FILE);
    let ended = next_of(&mut socket, "session-ended").await.unwrap();
    assert_eq!(ended["reason"], "ended");
    let deadline = Instant::now() + Duration::from_secs(3);
    while h.companion.state().running {
        assert!(
            Instant::now() < deadline,
            "the server outlived its last share"
        );
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    assert!(h.companion.shares().is_empty());
}

#[tokio::test]
async fn sharing_needs_a_live_authority_and_restarts_with_a_new_key() {
    let h = Harness::new("not-live");
    let refused = h.companion.share_start(FILE).err().unwrap();
    assert_eq!(refused.code, Code::NotLive);
    assert!(!h.companion.state().running);
    h.open(FILE, &document(vec![]));
    let first = h.share(FILE);
    let again = h.share(FILE);
    assert_eq!(first.token, again.token);
    h.companion.share_stop(FILE);
    h.companion.share_stop(FILE);
    assert!(!h.companion.state().running);
    let second = h.share(FILE);
    assert_ne!(first.id(), second.id());
    assert_ne!(first.secret(), second.secret());
    h.companion.stop_all(EndReason::Stopped);
}

#[tokio::test]
async fn secrets_stay_out_of_debug_events_and_endpoints() {
    let h = Harness::new("secrets");
    h.open(FILE, &document(vec![]));
    let s = h.share(FILE);
    let share = h.companion.core().grants.get(s.id()).unwrap();
    let debug = format!("{share:?} {share:#?}");
    assert!(
        debug.contains(s.id()) && !debug.contains(s.secret()),
        "{debug}"
    );
    let state = serde_json::to_string(&h.companion.state()).unwrap();
    assert!(state.contains(s.id()) && !state.contains(s.secret()));
    for event in h.states.lock().unwrap().iter() {
        assert!(!serde_json::to_string(event).unwrap().contains(s.secret()));
    }
    let endpoints = serde_json::to_value(h.companion.endpoints()).unwrap();
    assert!(!endpoints.to_string().contains(s.secret()));
    assert_eq!(
        endpoints["endpoints"][0]["url"],
        format!("http://{}:{}/c/", super::LAN, s.port)
    );
    let info = serde_json::to_value(&h.companion.shares()[0]).unwrap();
    let link = format!("http://{}:{}/c/#{}", super::LAN, s.port, s.token);
    assert_eq!(info["endpoints"][0]["url"], link.as_str());
    assert_eq!(info["endpoints"][0]["reachable"], false);
    assert_eq!(info["endpoints"][0]["label"], "Wi\u{2011}Fi/Ethernet en9");
}
