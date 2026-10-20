use super::client::{joined, next_json, next_of, post, socket};
use super::{add, document, shape, Harness, FILE};
use crate::companion::limits::{Limits, UNAUTHENTICATED_SOCKETS};
use futures_util::SinkExt;
use serde_json::json;
use std::time::{Duration, Instant};
use tokio_tungstenite::tungstenite::Message;

fn quick_hello() -> Limits {
    Limits {
        hello_timeout: Duration::from_millis(300),
        ..Limits::default()
    }
}

#[tokio::test]
async fn a_socket_without_hello_is_closed_after_the_timeout() {
    let h = Harness::with("hello-timeout", quick_hello(), 0);
    h.open(FILE, &document(vec![]));
    let s = h.share(FILE);
    let started = Instant::now();
    let mut quiet = socket(s.port, s.id()).await.unwrap();
    assert_eq!(next_json(&mut quiet).await, None);
    let waited = started.elapsed();
    assert!(waited >= Duration::from_millis(250), "{waited:?}");
    assert!(waited < Duration::from_secs(2), "{waited:?}");
    let mut wrong = socket(s.port, s.id()).await.unwrap();
    let hello = json!({"type": "hello", "clientId": "x", "secret": "nope"});
    wrong.send(Message::text(hello.to_string())).await.unwrap();
    assert_eq!(next_json(&mut wrong).await, None);
    let unknown = socket(s.port, &"0".repeat(32)).await;
    assert!(
        unknown.is_err(),
        "an unknown share is refused before the upgrade"
    );
}

#[tokio::test]
async fn sockets_hear_other_clients_commits_and_presence() {
    let h = Harness::new("push");
    h.open(FILE, &document(vec![shape("a", 0.0)]));
    let s = h.share(FILE);
    let (mut first, welcome) = joined(s.port, s.id(), s.secret(), "one").await;
    assert_eq!(welcome["epoch"], s.info.epoch.as_str());
    assert_eq!(
        (welcome["revision"].clone(), welcome["clients"].clone()),
        (json!(0), json!(1))
    );
    // The whole fragment works as the secret too.
    let (mut second, welcome) = joined(s.port, s.id(), &s.token, "two").await;
    assert_eq!(welcome["clients"], 2);
    assert_eq!(next_of(&mut first, "presence").await.unwrap()["clients"], 2);
    let commit = json!({"clientId": "two", "seq": 1, "opId": "two-1", "epoch": s.info.epoch, "transaction": add("b")});
    assert_eq!(
        post(s.port, &s.path("commit"), &s.bearer(), &commit)
            .await
            .status,
        200
    );
    let change = next_of(&mut first, "change").await.unwrap();
    assert_eq!(change["revision"], 1);
    assert_eq!(change["clientId"], "web:two");
    assert_eq!(change["epoch"], s.info.epoch.as_str());
    assert_eq!(change["transaction"]["edits"][0]["id"], "b");
    assert_eq!(next_of(&mut second, "change").await.unwrap()["revision"], 1);
    let mut big = shape("c", 0.0);
    big["text"] = json!("x".repeat(40 * 1024));
    let large = json!({"clientId": "two", "seq": 2, "opId": "two-2", "epoch": s.info.epoch,
        "transaction": {"edits": [{"id": "c", "before": null, "after": big}]}});
    assert_eq!(
        post(s.port, &s.path("commit"), &s.bearer(), &large)
            .await
            .status,
        200
    );
    let frame = next_json(&mut first).await.unwrap();
    assert_eq!(
        frame,
        json!({"type": "revision", "epoch": s.info.epoch, "revision": 2})
    );
    second.close(None).await.ok();
    assert_eq!(next_of(&mut first, "presence").await.unwrap()["clients"], 1);
    let states = h.states.lock().unwrap().clone();
    assert!(states
        .iter()
        .any(|state| state.shares.first().is_some_and(|share| share.clients == 2)));
}

#[tokio::test]
async fn unauthenticated_sockets_are_capped() {
    let h = Harness::new("unauthenticated");
    h.open(FILE, &document(vec![]));
    let s = h.share(FILE);
    let mut waiting = Vec::new();
    for _ in 0..UNAUTHENTICATED_SOCKETS {
        waiting.push(socket(s.port, s.id()).await.unwrap());
    }
    let refused = socket(s.port, s.id()).await;
    match refused {
        Err(tokio_tungstenite::tungstenite::Error::Http(response)) => {
            assert_eq!(response.status(), 503)
        }
        other => panic!("expected 503, got {:?}", other.map(|_| ())),
    }
    // Saying hello frees a place.
    let mut first = waiting.remove(0);
    let hello = json!({"type": "hello", "clientId": "x", "secret": s.secret()});
    first.send(Message::text(hello.to_string())).await.unwrap();
    assert_eq!(next_json(&mut first).await.unwrap()["type"], "welcome");
    assert!(socket(s.port, s.id()).await.is_ok());
}

#[tokio::test]
async fn a_pause_and_resume_reach_the_sockets() {
    use crate::library::files::canvases::authority::session::Resolution;
    let h = Harness::new("state");
    let rel = h.open(FILE, &document(vec![shape("a", 0.0)]));
    let s = h.share(FILE);
    let (mut socket, _) = joined(s.port, s.id(), s.secret(), "one").await;
    std::fs::write(
        h.root.join(&rel),
        crate::library::files::canvases::authority::format::serialize(&document(vec![])).unwrap(),
    )
    .unwrap();
    let commit = json!({"clientId": "one", "seq": 1, "opId": "one-1", "epoch": s.info.epoch, "transaction": add("b")});
    assert_eq!(
        post(s.port, &s.path("commit"), &s.bearer(), &commit)
            .await
            .status,
        423
    );
    assert_eq!(
        next_of(&mut socket, "state").await.unwrap()["phase"],
        "paused"
    );
    assert_eq!(
        h.companion.shares()[0].phase,
        crate::library::files::canvases::authority::events::Phase::Paused
    );
    h.authority
        .resolve_external(FILE, Resolution::AdoptDisk)
        .unwrap();
    assert_eq!(
        next_of(&mut socket, "state").await.unwrap()["phase"],
        "live"
    );
    assert_eq!(
        next_of(&mut socket, "change").await.unwrap()["clientId"],
        serde_json::Value::Null
    );
}

#[tokio::test]
async fn idle_sockets_get_a_text_ping_and_pongs_keep_them_alive() {
    let limits = Limits {
        ping_every: Duration::from_millis(150),
        dead_after: Duration::from_millis(600),
        ..Limits::default()
    };
    let h = Harness::with("ping", limits, 0);
    h.open(FILE, &document(vec![]));
    let s = h.share(FILE);
    let (mut socket, _) = joined(s.port, s.id(), s.secret(), "one").await;
    let started = Instant::now();
    // Answer each text ping as the page does, for longer than `dead_after`.
    while started.elapsed() < Duration::from_millis(1200) {
        let ping = next_of(&mut socket, "ping").await.expect("still open");
        assert_eq!(ping, json!({"type": "ping"}));
        socket
            .send(Message::text(json!({"type": "pong"}).to_string()))
            .await
            .unwrap();
    }
    assert_eq!(h.companion.shares()[0].clients, 1);
}
