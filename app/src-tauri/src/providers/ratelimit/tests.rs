use super::bucket::TokenBucket;
use super::retry::Retry;
use super::transport::transport_detail;

use std::time::{Duration, Instant};

#[test]
fn the_bucket_starts_full_and_then_paces() {
    let bucket = TokenBucket::new(600.0);
    let start = Instant::now();
    for _ in 0..600 {
        bucket.acquire();
    }
    assert!(
        start.elapsed() < Duration::from_millis(500),
        "a full bucket should not wait"
    );

    let paced = Instant::now();
    bucket.acquire();
    assert!(
        paced.elapsed() >= Duration::from_millis(50),
        "an empty bucket must wait"
    );
}

#[test]
fn a_cost_larger_than_a_whole_minute_does_not_deadlock() {
    let bucket = TokenBucket::new(6_000.0);
    let start = Instant::now();
    bucket.acquire_n(320_000.0);
    assert!(
        start.elapsed() < Duration::from_millis(200),
        "{:?}",
        start.elapsed()
    );
}

#[test]
fn retuning_narrows_the_ceiling_and_can_drain_the_burst() {
    let bucket = TokenBucket::new(60_000.0);
    bucket.retune(6_000.0, true);
    assert_eq!(bucket.per_minute(), 6_000.0);
    let start = Instant::now();
    bucket.acquire_n(100.0);
    // Drained: even a token has to be waited for.
    assert!(
        start.elapsed() >= Duration::from_millis(500),
        "{:?}",
        start.elapsed()
    );
}

#[test]
fn a_blocked_acquire_says_how_long_the_refill_takes_before_it_blocks() {
    let bucket = TokenBucket::new(6_000.0);
    assert_eq!(
        bucket.shortfall(100.0),
        Duration::ZERO,
        "a full bucket owes nothing"
    );
    bucket.retune(6_000.0, true);
    // 100 a second: 50 tokens is half a second away.
    let owed = bucket.shortfall(50.0);
    assert!(
        owed > Duration::from_millis(400) && owed <= Duration::from_millis(500),
        "{owed:?}"
    );

    let mut told = Vec::new();
    bucket.acquire_n_reporting(50.0, &mut |wait| told.push(wait));
    assert!(!told.is_empty());
    assert!(told[0] > Duration::from_millis(300), "{told:?}");
}

#[test]
fn the_ladder_allows_exactly_its_attempts() {
    let mut retry = Retry::new(4, 0.0);
    let mut backoffs = 0;
    while retry.attempts_left() && retry.back_off() {
        backoffs += 1;
    }
    assert_eq!(backoffs, 3);
}

#[test]
fn transport_detail_keeps_the_cause() {
    // A port nobody listens on: the cause is only in the source chain.
    let port = std::net::TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port();
    let url = format!("http://127.0.0.1:{port}/secret?signature=x");
    let Err(ureq::Error::Transport(transport)) = ureq::get(&url).call() else {
        panic!("expected a transport error");
    };
    let detail = transport_detail(&transport);
    assert!(detail.to_lowercase().contains("refused"), "{detail}");
    assert!(!detail.contains("signature"), "{detail}");
}
