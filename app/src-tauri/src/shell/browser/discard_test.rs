use std::time::{Duration, Instant};

use super::discard::{due, is_due, IDLE};
use super::{Inner, Tab};

const MINUTE: Duration = Duration::from_secs(60);

/// Tabs 1–3, all opened (backgrounded) at `at`.
fn three_tabs(at: Instant) -> Inner {
    let mut s = Inner::default();
    for id in 1..=3 {
        s.tabs
            .push(Tab::new(id, format!("https://example.com/{id}")));
        s.backgrounded.insert(id, at);
    }
    s
}

#[test]
fn a_tab_is_due_once_backgrounded_for_the_idle_time() {
    let opened = Instant::now();
    let s = three_tabs(opened);
    assert!(due(&s, opened + IDLE - MINUTE, IDLE).is_empty());
    assert_eq!(due(&s, opened + IDLE, IDLE), [1, 2, 3]);
    assert_eq!(due(&s, opened + IDLE * 4, IDLE), [1, 2, 3]);
}

#[test]
fn a_shown_tab_is_never_due_and_hiding_restarts_its_clock() {
    let opened = Instant::now();
    let mut s = three_tabs(opened);
    s.show(1);
    assert_eq!(due(&s, opened + IDLE * 2, IDLE), [2, 3]);

    let hidden = opened + IDLE * 2;
    s.hide(1, hidden);
    assert!(!is_due(&s, 1, hidden + IDLE - MINUTE, IDLE));
    assert!(is_due(&s, 1, hidden + IDLE, IDLE));
}

#[test]
fn a_discarded_or_closed_tab_is_not_due_again() {
    let opened = Instant::now();
    let mut s = three_tabs(opened);
    s.discarded.insert(2);
    s.forget(3);
    assert_eq!(due(&s, opened + IDLE, IDLE), [1]);
    assert!(!is_due(&s, 3, opened + IDLE, IDLE));
}

#[test]
fn hiding_a_closed_tab_records_nothing() {
    let mut s = three_tabs(Instant::now());
    s.forget(2);
    s.hide(2, Instant::now());
    assert!(!s.backgrounded.contains_key(&2));
}

#[test]
fn memory_pressure_takes_every_backgrounded_tab_at_once() {
    let opened = Instant::now();
    let mut s = three_tabs(opened);
    s.show(3);
    assert_eq!(due(&s, opened, Duration::ZERO), [1, 2]);
}

#[test]
fn a_clock_behind_the_hide_time_is_not_due() {
    let hidden = Instant::now() + MINUTE;
    let s = three_tabs(hidden);
    assert!(due(&s, hidden - MINUTE, IDLE).is_empty());
}
