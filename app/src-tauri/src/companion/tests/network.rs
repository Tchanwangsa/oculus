//! The address list moving under a running server: links and the Host
//! allow-list follow it, and nothing a device holds is disturbed.

use super::client::{get, next_json, next_of, open_response, Socket};
use super::{add, addr, document, image_document, Harness, FILE, LAN, WINDOW};
use crate::companion::grants::EndReason;
use crate::companion::wire::CompanionState;
use futures_util::SinkExt;
use serde_json::json;
use std::collections::BTreeSet;
use std::time::{Duration, Instant};
use tokio::io::AsyncReadExt;
use tokio::net::TcpStream;
use tokio_tungstenite::tungstenite::Message;

const HOTSPOT: &str = "172.20.10.2";
const VPN: &str = "10.8.0.2";

/// A socket whose Host names `host` (connected over loopback), past `hello`.
async fn joined_via(host: &str, port: u16, share_id: &str, secret: &str, client: &str) -> Socket {
    let stream = TcpStream::connect(("127.0.0.1", port)).await.unwrap();
    let url = format!("ws://{host}:{port}/api/s/{share_id}/ws");
    let (mut socket, _) = tokio_tungstenite::client_async(url, stream).await.unwrap();
    let hello = json!({"type": "hello", "clientId": client, "secret": secret});
    socket.send(Message::text(hello.to_string())).await.unwrap();
    let welcome = next_json(&mut socket).await.expect("welcome");
    assert_eq!(welcome["type"], "welcome", "{welcome}");
    socket
}

async fn until(what: &str, done: impl Fn() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(3);
    while !done() {
        assert!(Instant::now() < deadline, "timed out waiting for {what}");
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
}

fn last_state(h: &Harness) -> CompanionState {
    h.states.lock().unwrap().last().unwrap().clone()
}

fn addresses(state: &CompanionState) -> Vec<String> {
    state.endpoints.iter().map(|e| e.address.clone()).collect()
}

async fn page(port: u16, ip: &str) -> u16 {
    let host = format!("{ip}:{port}");
    get(port, "/c/", &[("Host", &host)]).await.status
}

#[tokio::test]
async fn an_address_change_keeps_the_share_its_key_sockets_and_requests() {
    let h = Harness::new("net-swap");
    h.open(FILE, &image_document(8 * 1024 * 1024));
    let s = h.share(FILE);
    h.net.watching().await;
    let mut socket = joined_via(LAN, s.port, s.id(), s.secret(), "ipad").await;
    let lan_host = format!("{LAN}:{}", s.port);
    let (mut pending, head) = open_response(
        s.port,
        &s.path("snapshot"),
        &[("Authorization", &s.bearer()), ("Host", &lan_host)],
    )
    .await;
    assert_eq!(head.status, 200);
    let length: usize = head.header("content-length").unwrap().parse().unwrap();
    let before = h.companion.state();

    // Wi-Fi gives way to a phone's hotspot.
    h.net.set(vec![addr(HOTSPOT, "en0")]);
    h.net.down(LAN);
    h.net.up(HOTSPOT);
    until("a state for the new list", || {
        last_state(&h).generation > before.generation
    })
    .await;

    let after = last_state(&h);
    assert_eq!(addresses(&after), [HOTSPOT]);
    assert!(after.running);
    assert_eq!(after.port, before.port, "the listener stays");
    let shares = h.companion.shares();
    assert_eq!(shares.len(), 1);
    let info = &shares[0];
    assert_eq!(
        (info.share_id.as_str(), &info.epoch),
        (s.id(), &s.info.epoch)
    );
    assert_eq!(info.generation, after.generation);
    assert_eq!(
        info.endpoints[0].url.as_deref(),
        Some(format!("http://{HOTSPOT}:{}/c/#{}", s.port, s.token).as_str()),
        "the same key, on the new address"
    );
    assert_eq!(info.endpoints[0].id, format!("en0/{HOTSPOT}"));

    // The socket is still live and still gets changes.
    h.desktop_commit(FILE, 1, add("b"));
    let change = next_of(&mut socket, "change")
        .await
        .expect("still connected");
    assert_eq!(change["revision"], 1);
    // The snapshot that was mid-transfer finishes.
    let mut read = head.body.len();
    let mut buf = vec![0u8; 256 * 1024];
    while read < length {
        let n = tokio::time::timeout(Duration::from_secs(5), pending.read(&mut buf))
            .await
            .expect("the snapshot kept flowing")
            .unwrap();
        assert!(n > 0, "the snapshot was cut at {read} of {length}");
        read += n;
    }
    assert_eq!(
        h.companion.core().grants.get(s.id()).unwrap().token(),
        s.token
    );
    h.companion.stop_all(EndReason::Stopped);
}

#[tokio::test]
async fn the_host_allow_list_follows_the_list() {
    let h = Harness::new("net-hosts");
    h.open(FILE, &document(vec![]));
    let s = h.share(FILE);
    h.net.watching().await;
    assert_eq!(page(s.port, LAN).await, 200);
    assert_eq!(page(s.port, HOTSPOT).await, 403);

    h.net.set(vec![addr(HOTSPOT, "en0")]);
    h.net.down(LAN);
    h.net.up(HOTSPOT);
    until("the swap", || addresses(&last_state(&h)) == [HOTSPOT]).await;
    assert_eq!(
        page(s.port, LAN).await,
        403,
        "a vanished address is refused"
    );
    assert_eq!(page(s.port, HOTSPOT).await, 200);
    assert_eq!(page(s.port, "127.0.0.1").await, 200, "loopback always");

    // An address the observer has not settled yet: a miss re-reads.
    tokio::time::sleep(Duration::from_millis(300)).await;
    h.net.set(vec![addr(HOTSPOT, "en0"), addr(VPN, "utun4")]);
    assert_eq!(page(s.port, VPN).await, 200);
    h.companion.stop_all(EndReason::Stopped);
}

#[tokio::test]
async fn a_vpn_comes_and_goes_beside_a_stable_wifi_endpoint() {
    let h = Harness::new("net-vpn");
    h.open(FILE, &document(vec![]));
    let s = h.share(FILE);
    h.net.watching().await;
    let wifi = format!("en9/{LAN}");

    h.net.set(vec![addr(LAN, "en9"), addr(VPN, "utun4")]);
    h.net.up(VPN);
    until("the VPN", || addresses(&last_state(&h)).len() == 2).await;
    let info = &h.companion.shares()[0];
    let ids: BTreeSet<&str> = info.endpoints.iter().map(|e| e.id.as_str()).collect();
    assert_eq!(ids, BTreeSet::from([wifi.as_str(), "utun4/10.8.0.2"]));
    let vpn = info
        .endpoints
        .iter()
        .find(|e| e.interface == "utun4")
        .unwrap();
    assert_eq!(vpn.label, "VPN/tunnel utun4");
    assert_eq!(page(s.port, VPN).await, 200);

    h.net.set(vec![addr(LAN, "en9")]);
    h.net.down(VPN);
    until("the VPN to go", || addresses(&last_state(&h)).len() == 1).await;
    let info = &h.companion.shares()[0];
    assert_eq!(info.endpoints.len(), 1);
    assert_eq!(
        info.endpoints[0].id, wifi,
        "the Wi-Fi endpoint keeps its id"
    );
    assert_eq!(page(s.port, VPN).await, 403);
    h.companion.stop_all(EndReason::Stopped);
}

#[tokio::test]
async fn a_burst_of_twenty_edges_is_at_most_two_states() {
    let h = Harness::new("net-burst");
    h.open(FILE, &document(vec![]));
    let s = h.share(FILE);
    h.net.watching().await;
    let start = h.companion.state().generation;
    for n in 0..20 {
        let ip = if n % 2 == 0 { HOTSPOT } else { LAN };
        h.net.set(vec![addr(ip, "en0")]);
        h.net.up(ip);
    }
    h.net.set(vec![addr(HOTSPOT, "en0")]);
    until("the burst to settle", || last_state(&h).generation > start).await;
    tokio::time::sleep(WINDOW * 3).await;
    let generations: BTreeSet<u64> = h
        .states
        .lock()
        .unwrap()
        .iter()
        .map(|state| state.generation)
        .filter(|g| *g > start)
        .collect();
    assert!(generations.len() <= 2, "{generations:?}");
    assert_eq!(addresses(&last_state(&h)), [HOTSPOT]);
    assert!(h.companion.state().running && s.port > 0);
    h.companion.stop_all(EndReason::Stopped);
}

#[tokio::test]
async fn an_endpoint_counts_the_devices_that_came_in_through_it() {
    let h = Harness::new("net-connected");
    h.open(FILE, &document(vec![]));
    let s = h.share(FILE);
    h.net.watching().await;
    assert!(!h.companion.shares()[0].endpoints[0].reachable);
    let socket = joined_via(LAN, s.port, s.id(), s.secret(), "ipad").await;
    let _loopback = super::client::joined(s.port, s.id(), s.secret(), "mac").await;
    let info = &h.companion.shares()[0];
    assert_eq!(info.clients, 2);
    let lan = &info.endpoints[0];
    assert_eq!((lan.connected_clients, lan.reachable), (1, true));
    let bare = &h.companion.endpoints().endpoints[0];
    assert_eq!((bare.connected_clients, bare.reachable), (1, true));
    assert_eq!(last_state(&h).endpoints[0].connected_clients, 1);
    drop(socket);
    until("the device to leave", || {
        h.companion.shares()[0].endpoints[0].connected_clients == 0
    })
    .await;
    assert!(!h.companion.shares()[0].endpoints[0].reachable);
    h.companion.stop_all(EndReason::Stopped);
}

#[tokio::test]
async fn stopping_the_server_stops_following_the_network() {
    let h = Harness::new("net-stop");
    h.open(FILE, &document(vec![]));
    h.share(FILE);
    h.net.watching().await;
    h.companion.stop_all(EndReason::Stopped);
    let count = h.states.lock().unwrap().len();
    h.net.set(vec![addr(HOTSPOT, "en0")]);
    h.net.up(HOTSPOT);
    tokio::time::sleep(WINDOW * 3).await;
    assert_eq!(
        h.states.lock().unwrap().len(),
        count,
        "no state while stopped"
    );
}
