//! Nudges the session task when a network address appears, so a reconnect
//! waiting in backoff tries as soon as the network is back. The addresses
//! come from the process's one observer (`runtime::network`), whose
//! starting addresses are never reported as appearing. The nudge is
//! `music_wake`'s: an up link only checks its session, so a healthy one is
//! never torn down for a network change. Subscribed once per app run, with
//! the first session.

use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use tauri::AppHandle;
use tokio::sync::mpsc::{unbounded_channel, UnboundedReceiver};

use super::state;
use crate::runtime::network::{self, Event, Observer};

/// Interfaces flap as Wi-Fi joins: one nudge per burst, and one more if it
/// went on past this.
const DEBOUNCE: Duration = Duration::from_secs(1);

static STARTED: AtomicBool = AtomicBool::new(false);

/// Starts following the network once per app run.
pub(super) fn start(app: &AppHandle) {
    if STARTED.swap(true, Ordering::AcqRel) {
        return;
    }
    let app = app.clone();
    follow(network::global(), DEBOUNCE, move || {
        state::wake(&app);
    });
}

/// Calls `wake` per burst of appearing addresses on `observer`, for the life
/// of the process.
fn follow(observer: &Observer, debounce: Duration, wake: impl Fn() + Send + 'static) {
    let (ups, received) = unbounded_channel();
    // An address going away nudges nothing: an attempt then would only fail
    // and lengthen the backoff.
    let subscription = observer.subscribe(move |event| {
        if let Event::Up(_) = event {
            ups.send(()).ok();
        }
    });
    tauri::async_runtime::spawn(async move {
        let _subscription = subscription;
        nudges(received, debounce, wake).await;
    });
}

/// One nudge at a burst's first appearance, and one more if others followed
/// within `debounce`.
async fn nudges(mut ups: UnboundedReceiver<()>, debounce: Duration, wake: impl Fn()) {
    while ups.recv().await.is_some() {
        wake();
        let quiet = tokio::time::sleep(debounce);
        tokio::pin!(quiet);
        let mut more = false;
        loop {
            tokio::select! {
                _ = &mut quiet => break,
                up = ups.recv() => match up {
                    Some(()) => more = true,
                    None => return,
                },
            }
        }
        if more {
            wake();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime::network::fake::{addr, fake};
    use std::sync::atomic::AtomicUsize;
    use std::sync::Arc;

    const WINDOW: Duration = Duration::from_millis(80);

    fn counted() -> (Arc<AtomicUsize>, impl Fn() + Send + 'static) {
        let wakes = Arc::new(AtomicUsize::new(0));
        let counter = wakes.clone();
        (wakes, move || {
            counter.fetch_add(1, Ordering::SeqCst);
        })
    }

    #[tokio::test]
    async fn a_down_alone_does_not_wake_and_an_up_does() {
        let (observer, net) = fake(vec![addr("192.168.1.4", "en0")], WINDOW);
        let (wakes, wake) = counted();
        follow(&observer, WINDOW, wake);
        net.watching().await;
        net.set(vec![]);
        net.down("192.168.1.4");
        tokio::time::sleep(WINDOW * 3).await;
        assert_eq!(wakes.load(Ordering::SeqCst), 0, "a removal never nudges");
        net.set(vec![addr("192.168.1.9", "en0")]);
        net.up("192.168.1.9");
        tokio::time::sleep(WINDOW / 4).await;
        assert_eq!(
            wakes.load(Ordering::SeqCst),
            1,
            "the first up nudges at once"
        );
        tokio::time::sleep(WINDOW * 3).await;
        assert_eq!(
            wakes.load(Ordering::SeqCst),
            1,
            "no trailing nudge for one up"
        );
    }

    #[tokio::test]
    async fn a_burst_nudges_once_then_once_more_after_the_window() {
        let (observer, net) = fake(vec![addr("192.168.1.4", "en0")], WINDOW);
        let (wakes, wake) = counted();
        follow(&observer, WINDOW, wake);
        net.watching().await;
        for ip in ["192.168.1.9", "fd00::9", "10.0.0.9", "192.168.1.9"] {
            net.up(ip);
            net.down("192.168.1.4");
        }
        tokio::time::sleep(WINDOW / 4).await;
        assert_eq!(wakes.load(Ordering::SeqCst), 1);
        tokio::time::sleep(WINDOW * 3).await;
        assert_eq!(wakes.load(Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn the_starting_addresses_never_wake() {
        let (observer, net) = fake(vec![addr("192.168.1.4", "en0")], WINDOW);
        net.up("192.168.1.4");
        net.up("fd00::4");
        let (wakes, wake) = counted();
        follow(&observer, WINDOW, wake);
        net.watching().await;
        tokio::time::sleep(WINDOW * 3).await;
        assert_eq!(wakes.load(Ordering::SeqCst), 0);
    }
}
