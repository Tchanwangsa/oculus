//! One live run of the real `agy` through `start` / `submit_code` / `cancel`:
//! the menu answered, the URL caught, a bogus code refused, then cancelled.
//! Never completes a sign-in.
//!
//! Ignored, and refuses to run unless `HOME` is a scratch home named again in
//! `OCULUS_AGY_SMOKE_HOME`, so the student's own signed-in agy is never
//! started. Put a logging `open` first on `PATH` so no browser tab opens, and
//! point `OCULUS_ANTIGRAVITY_BIN` at agy (discovery looks under `HOME`):
//!
//! ```text
//! HOME=$S/agyhome OCULUS_AGY_SMOKE_HOME=$S/agyhome PATH=$S/shim:$PATH \
//!   OCULUS_ANTIGRAVITY_BIN=~/.local/bin/agy \
//!   target/debug/deps/app_lib-… agy::smoke --ignored --nocapture
//! ```

use std::sync::mpsc;
use std::time::Duration;

use super::super::{cancel, start, submit_code, SignInLine};
use crate::harness::event::Provider;

const P: Provider = Provider::Antigravity;
const WAIT: Duration = Duration::from_secs(60);

/// The next event that `want` accepts, printing every one on the way.
fn until(rx: &mpsc::Receiver<SignInLine>, what: &str, want: impl Fn(&SignInLine) -> bool) {
    loop {
        let ev = rx
            .recv_timeout(WAIT)
            .unwrap_or_else(|e| panic!("no {what} within {WAIT:?}: {e}"));
        println!("event: {}", describe(&ev));
        if want(&ev) {
            return;
        }
        assert!(!ev.done, "the run ended before {what}: {:?}", ev.status);
    }
}

fn describe(ev: &SignInLine) -> String {
    if let Some(l) = &ev.line {
        format!("line {l:?}")
    } else if let Some(u) = &ev.url {
        format!("url {}…({} chars)", &u[..u.len().min(60)], u.len())
    } else if let Some(n) = &ev.notice {
        format!("notice {n:?}")
    } else {
        format!("done ok={:?} status={:?}", ev.ok, ev.status)
    }
}

#[test]
#[ignore = "runs the real agy; see the module docs"]
fn a_bogus_code_is_refused_and_cancel_ends_the_run() {
    let home = std::env::var("HOME").unwrap_or_default();
    let guard = std::env::var("OCULUS_AGY_SMOKE_HOME").unwrap_or_default();
    assert!(
        !home.is_empty() && home == guard,
        "HOME must be a scratch home, named again in OCULUS_AGY_SMOKE_HOME"
    );

    let (tx, rx) = mpsc::channel();
    start(P, move |ev| tx.send(ev).unwrap()).unwrap();

    until(&rx, "the authorize URL", |ev| {
        ev.url
            .as_deref()
            .is_some_and(|u| u.starts_with("https://accounts.google.com/"))
    });
    // The code box comes up right after the URL; wait until it takes a code.
    let mut sent = false;
    for _ in 0..100 {
        if submit_code(P, "bogus-not-a-real-code").is_ok() {
            sent = true;
            break;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    assert!(sent, "agy never showed its code box");

    until(&rx, "the refusal", |ev| {
        ev.notice
            .as_deref()
            .is_some_and(|n| n.contains("token exchange failed"))
    });
    // The next round's URL: the run carries on after a refusal.
    until(&rx, "a second URL", |ev| ev.url.is_some());

    cancel(P).unwrap();
    loop {
        let ev = rx.recv_timeout(WAIT).expect("no done after cancel");
        println!("event: {}", describe(&ev));
        if ev.done {
            assert_eq!(ev.ok, Some(false));
            assert_eq!(ev.status.as_deref(), Some("cancelled"));
            break;
        }
    }
}
