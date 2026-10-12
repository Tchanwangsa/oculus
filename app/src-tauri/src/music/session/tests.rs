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
fn a_tick_whose_wall_time_ran_ahead_of_its_awake_time_means_the_mac_just_woke() {
    // One poll tick apart: a second on both clocks is no sleep.
    let tick = Duration::from_secs(1);
    assert!(!slept(tick, tick));
    // A night between two ticks, about a second of it awake.
    assert!(slept(Duration::from_secs(8 * 3600), tick));
    // A tick held up while awake is late on both clocks: no sleep.
    let late = Duration::from_secs(100);
    assert!(!slept(late, late));
    // Clocks drift by a few seconds; that is not sleep.
    assert!(!slept(tick + Duration::from_secs(5), tick));
    assert!(!slept(tick + SLEPT_AFTER, tick));
    assert!(slept(tick + SLEPT_AFTER + Duration::from_secs(1), tick));
    // A wall clock set back is not sleep.
    assert!(!slept(Duration::ZERO, tick));
}
