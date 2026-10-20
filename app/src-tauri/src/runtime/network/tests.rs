use super::fake::{addr, fake};
use super::{Event, LocalAddress};
use std::net::IpAddr;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

const WINDOW: Duration = Duration::from_millis(60);

fn recorder(observer: &super::Observer) -> (super::Subscription, Arc<Mutex<Vec<Event>>>) {
    let events = Arc::new(Mutex::new(Vec::new()));
    let sink = events.clone();
    let subscription = observer.subscribe(move |event| sink.lock().unwrap().push(event.clone()));
    (subscription, events)
}

fn changes(events: &Mutex<Vec<Event>>) -> Vec<u64> {
    events
        .lock()
        .unwrap()
        .iter()
        .filter_map(|event| match event {
            Event::Changed(snapshot) => Some(snapshot.generation),
            _ => None,
        })
        .collect()
}

/// Waits until `done` holds, or panics after two seconds.
async fn until(what: &str, done: impl Fn() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(2);
    while !done() {
        assert!(Instant::now() < deadline, "timed out waiting for {what}");
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
}

fn ips(addrs: &[LocalAddress]) -> Vec<String> {
    addrs.iter().map(|a| a.ip.to_string()).collect()
}

#[tokio::test]
async fn starts_lazily_with_a_first_snapshot() {
    let (observer, net) = fake(vec![addr("192.168.1.4", "en0")], WINDOW);
    observer.refresh();
    assert_eq!(net.reads(), 0, "refresh before start reads nothing");
    let first = observer.snapshot();
    assert_eq!(first.generation, 1);
    assert_eq!(ips(&first.addrs), ["192.168.1.4"]);
    assert_eq!(observer.snapshot(), first);
}

#[tokio::test]
async fn additions_and_removals_both_move_the_generation() {
    let (observer, net) = fake(vec![addr("192.168.1.4", "en0")], WINDOW);
    let (_subscription, events) = recorder(&observer);
    net.watching().await;
    net.set(vec![addr("192.168.1.4", "en0"), addr("10.8.0.2", "utun4")]);
    net.up("10.8.0.2");
    until("the VPN address", || changes(&events) == [2]).await;
    net.set(vec![addr("192.168.1.4", "en0")]);
    net.down("10.8.0.2");
    until("the VPN address to go", || changes(&events) == [2, 3]).await;
    let snapshot = observer.snapshot();
    assert_eq!(
        (snapshot.generation, ips(&snapshot.addrs)),
        (3, vec!["192.168.1.4".into()])
    );
    let edges: Vec<Event> = events
        .lock()
        .unwrap()
        .iter()
        .filter(|e| !matches!(e, Event::Changed(_)))
        .cloned()
        .collect();
    let vpn: IpAddr = "10.8.0.2".parse().unwrap();
    assert!(matches!(edges[..], [Event::Up(a), Event::Down(b)] if a == vpn && b == vpn));
}

#[tokio::test]
async fn a_burst_of_edges_is_one_or_two_snapshots() {
    let (observer, net) = fake(vec![addr("172.20.10.2", "en0")], WINDOW);
    let (_subscription, events) = recorder(&observer);
    net.watching().await;
    for n in 0..20 {
        let ip = if n % 2 == 0 {
            "192.168.1.4"
        } else {
            "172.20.10.2"
        };
        net.set(vec![addr(ip, "en0")]);
        net.up(ip);
    }
    net.set(vec![addr("192.168.1.4", "en0")]);
    net.down("172.20.10.2");
    until("the burst to settle", || !changes(&events).is_empty()).await;
    tokio::time::sleep(WINDOW * 3).await;
    let changed = changes(&events);
    assert!(changed.len() <= 2, "{changed:?}");
    assert_eq!(ips(&observer.snapshot().addrs), ["192.168.1.4"]);
}

#[tokio::test]
async fn an_edge_that_changes_nothing_keeps_the_generation() {
    let (observer, net) = fake(vec![addr("192.168.1.4", "en0")], WINDOW);
    let (_subscription, events) = recorder(&observer);
    net.watching().await;
    net.up("192.168.1.4");
    until("the edge", || !events.lock().unwrap().is_empty()).await;
    tokio::time::sleep(WINDOW * 3).await;
    assert!(changes(&events).is_empty());
    assert_eq!(observer.snapshot().generation, 1);
}

#[tokio::test]
async fn refresh_publishes_at_once_and_unsubscribing_stops_events() {
    let (observer, net) = fake(vec![addr("192.168.1.4", "en0")], WINDOW);
    let (subscription, events) = recorder(&observer);
    net.watching().await;
    net.set(vec![addr("192.168.1.9", "en0")]);
    observer.refresh();
    assert_eq!(
        changes(&events),
        [2],
        "refresh publishes on the caller's thread"
    );
    observer.refresh();
    assert_eq!(changes(&events), [2], "nothing moved");
    drop(subscription);
    net.set(vec![addr("192.168.1.10", "en0")]);
    observer.refresh();
    assert_eq!(changes(&events), [2]);
    assert_eq!(observer.snapshot().generation, 3);
}

#[tokio::test]
async fn the_starting_burst_is_not_reported_as_edges() {
    let (observer, net) = fake(vec![addr("192.168.1.4", "en0")], WINDOW);
    net.up("192.168.1.4");
    net.up("fd00::4");
    let (_subscription, events) = recorder(&observer);
    net.watching().await;
    tokio::time::sleep(WINDOW * 2).await;
    assert!(
        events.lock().unwrap().is_empty(),
        "{:?}",
        events.lock().unwrap()
    );
    net.up("192.168.1.4");
    until("a later edge", || events.lock().unwrap().len() == 1).await;
}

#[tokio::test]
async fn a_stream_error_leaves_refresh_working() {
    let (observer, net) = fake(vec![addr("192.168.1.4", "en0")], WINDOW);
    let (_subscription, events) = recorder(&observer);
    net.watching().await;
    net.fail_stream();
    tokio::time::sleep(WINDOW).await;
    net.set(vec![addr("192.168.1.9", "en0")]);
    net.up("192.168.1.9");
    tokio::time::sleep(WINDOW * 3).await;
    assert!(events.lock().unwrap().is_empty(), "the stream has stopped");
    observer.refresh();
    assert_eq!(changes(&events), [2]);
}

#[tokio::test]
async fn an_unreadable_list_keeps_the_last_one() {
    let (observer, net) = fake(vec![addr("192.168.1.4", "en0")], WINDOW);
    let first = observer.snapshot();
    net.fail_reads(true);
    net.set(vec![]);
    observer.refresh();
    assert_eq!(observer.snapshot(), first);
    net.fail_reads(false);
    observer.refresh();
    assert_eq!(observer.snapshot().generation, 2);
}

#[test]
fn usable_addresses_exclude_loopback_down_and_link_local() {
    let down = super::LocalAddress::new("en1", "192.168.1.5".parse().unwrap(), false, false);
    for (address, usable) in [
        (addr("192.168.1.4", "en0"), true),
        (addr("100.86.3.4", "en0"), true),
        (addr("fd00::4", "en0"), true),
        (addr("127.0.0.1", "lo0"), false),
        (addr("::1", "lo0"), false),
        (addr("169.254.3.1", "en0"), false),
        (addr("fe80::1", "en0"), false),
        (addr("febf::1", "en0"), false),
        (down, false),
    ] {
        assert_eq!(address.usable(), usable, "{address:?}");
    }
}

#[test]
fn the_real_list_reads_and_never_offers_loopback_or_link_local() {
    use super::Provider;
    for a in super::Interfaces.read().unwrap() {
        if a.loopback || a.link_local {
            assert!(!a.usable(), "{a:?}");
        }
    }
}
