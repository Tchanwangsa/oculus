//! The manager's session map is never held across a call into a CLI.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::channel;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use crate::harness::testkit::outbound::Wire;
use crate::harness::{Harness, OneSink, Provider, SendOptions, Sinks};

use super::guard::{assert_scratch, scratch};

/// A CLI that takes its time over a message (Codex answers `turn/start` in
/// up to a minute) must not hold up another thread's send, nor a Stop.
#[test]
fn a_send_waiting_on_its_cli_holds_up_no_other_thread() {
    let dir = scratch("turn-locks");
    let harness = Arc::new(Harness::replay(assert_scratch(&dir, &dir)));
    let sinks: Arc<dyn Sinks> = Arc::new(OneSink(Arc::new(|_| {})));

    // Thread 1's CLI sits on the first line written to it until let go.
    let (entered, waiting) = channel();
    let (let_go, held) = channel::<()>();
    let (held, first) = (Mutex::new(held), AtomicBool::new(true));
    let slow: Wire = Arc::new(move |_| {
        if first.swap(false, Ordering::SeqCst) {
            entered.send(()).unwrap();
            let _ = held.lock().unwrap().recv();
        }
        Ok(())
    });
    let quick: Wire = Arc::new(|_| Ok(()));
    for (thread, wire) in [(1, slow), (2, quick)] {
        let bridge = crate::harness::claude::replay::bridge(wire, Arc::new(|_| {}));
        harness.replay_spawn(thread, move |_| bridge.handle);
    }

    let send = |thread: i64| {
        let (harness, sinks) = (harness.clone(), sinks.clone());
        std::thread::spawn(move || {
            harness.send(
                thread,
                Provider::Claude,
                None,
                &SendOptions::default(),
                "question",
                &sinks,
            )
        })
    };
    let stuck = send(1);
    waiting
        .recv_timeout(Duration::from_secs(10))
        .expect("thread 1's send reached its CLI");

    let (done, others) = channel();
    let other = harness.clone();
    let second = send(2);
    std::thread::spawn(move || {
        let sent = second.join().unwrap();
        let stopped = other.interrupt(2).and_then(|()| other.interrupt(1));
        let _ = done.send(sent.and(stopped));
    });
    let outcome = others.recv_timeout(Duration::from_secs(5));
    let_go.send(()).unwrap();
    stuck.join().unwrap().unwrap();
    outcome
        .expect("another thread's send, and both Stops, went through meanwhile")
        .unwrap();
}
