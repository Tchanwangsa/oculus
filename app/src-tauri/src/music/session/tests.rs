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
fn a_long_gap_between_ticks_on_either_clock_means_the_machine_just_woke() {
    let tick = Duration::from_secs(1);
    let night = Duration::from_secs(8 * 3600);
    // Normal ticks, a second apart on both clocks, or a few seconds late.
    assert!(!slept(tick, tick));
    assert!(!slept(
        tick + Duration::from_secs(5),
        tick + Duration::from_secs(5)
    ));
    assert!(!slept(SLEPT_AFTER, SLEPT_AFTER));
    // macOS: `Instant` stopped through the sleep, the wall clock did not.
    assert!(slept(night, tick));
    assert!(slept(SLEPT_AFTER + tick, tick));
    // Windows: `Instant` counted through suspend too.
    assert!(slept(night, night));
    // A wall clock set back reads as no wall time, and is no sleep by itself.
    assert!(!slept(Duration::ZERO, tick));
    assert!(slept(Duration::ZERO, night));
}
