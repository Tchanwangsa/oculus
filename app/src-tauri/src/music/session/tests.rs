use std::time::Duration;

use super::policy::{slept, Backoff, BACKOFF_MAX, SLEPT_AFTER, STABLE_AFTER};

fn secs(d: Duration) -> u64 {
    d.as_secs()
}

#[test]
fn failed_attempts_back_off_from_two_seconds_to_a_minute() {
    let mut backoff = Backoff::default();
    let seen: Vec<u64> = (0..8).map(|_| secs(backoff.after_failure())).collect();
    assert_eq!(seen, [2, 4, 8, 16, 32, 60, 60, 60]);
    assert_eq!(BACKOFF_MAX, Duration::from_secs(60));
}

#[test]
fn a_stable_connection_that_drops_reconnects_at_once_and_starts_over() {
    let mut backoff = Backoff::default();
    for _ in 0..4 {
        backoff.after_failure();
    }
    assert_eq!(backoff.after_drop(STABLE_AFTER), Duration::ZERO);
    // The attempts after it back off from the start.
    assert_eq!(secs(backoff.after_failure()), 2);
    assert_eq!(secs(backoff.after_failure()), 4);
    assert_eq!(
        backoff.after_drop(Duration::from_secs(1680)),
        Duration::ZERO
    );
    assert_eq!(secs(backoff.after_failure()), 2);
}

#[test]
fn a_connection_that_keeps_dropping_at_once_backs_off() {
    let mut backoff = Backoff::default();
    let short = Duration::from_secs(5);
    let seen: Vec<u64> = (0..4).map(|_| secs(backoff.after_drop(short))).collect();
    assert_eq!(seen, [2, 4, 8, 16]);
}

#[test]
fn wall_time_running_ahead_of_awake_time_means_the_mac_slept() {
    let awake = Duration::from_secs(100);
    assert!(!slept(awake, awake));
    // Clocks drift and wake late by a few seconds; that is not sleep.
    assert!(!slept(awake + Duration::from_secs(5), awake));
    assert!(!slept(awake + SLEPT_AFTER, awake));
    assert!(slept(awake + SLEPT_AFTER + Duration::from_secs(1), awake));
    assert!(slept(
        Duration::from_secs(8 * 3600),
        Duration::from_secs(20)
    ));
    // A wall clock set back is not sleep.
    assert!(!slept(Duration::ZERO, awake));
}
