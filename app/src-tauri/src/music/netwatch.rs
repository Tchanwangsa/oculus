//! Watches this machine's network addresses (`if-watch`: SCDynamicStore on
//! macOS, the IP helper on Windows, netlink on Linux) and nudges the session
//! task when one appears, so a reconnect waiting in backoff tries as soon as
//! the network is back. The nudge is `music_wake`'s: an up link only checks
//! its session, so a healthy one is never torn down for a network change.
//! One watcher for the app, started with the first session; macOS's watcher
//! thread outlives a dropped watcher, so it is never restarted.

use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use futures_util::{FutureExt, StreamExt};
use if_watch::tokio::IfWatcher;
use if_watch::IfEvent;
use tauri::AppHandle;

use super::state;

/// Interfaces flap as Wi-Fi joins: one nudge per burst, and one more if it
/// went on past this.
const DEBOUNCE: Duration = Duration::from_secs(1);

static STARTED: AtomicBool = AtomicBool::new(false);

/// Starts the watcher once per app run.
pub(super) fn start(app: &AppHandle) {
    if STARTED.swap(true, Ordering::AcqRel) {
        return;
    }
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        let mut watcher = match IfWatcher::new() {
            Ok(watcher) => watcher,
            Err(why) => {
                elog!(
                    "[music] could not watch the network; reconnects keep to their backoff: {why}"
                );
                return;
            }
        };
        // The addresses there at the start arrive as events at once.
        while let Some(Some(_)) = watcher.next().now_or_never() {}
        loop {
            if !next_up(&mut watcher).await {
                return;
            }
            state::wake(&app);
            let quiet = tokio::time::sleep(DEBOUNCE);
            tokio::pin!(quiet);
            let mut more = false;
            loop {
                tokio::select! {
                    _ = &mut quiet => break,
                    event = watcher.next() => match event {
                        Some(Ok(IfEvent::Up(_))) => more = true,
                        Some(Ok(IfEvent::Down(_))) => {}
                        Some(Err(why)) => return stopped(&why),
                        None => return,
                    },
                }
            }
            if more {
                state::wake(&app);
            }
        }
    });
}

/// Waits for an address to appear; false once the watcher has stopped. An
/// address going away nudges nothing: an attempt then would only fail and
/// lengthen the backoff.
async fn next_up(watcher: &mut IfWatcher) -> bool {
    loop {
        match watcher.next().await {
            Some(Ok(IfEvent::Up(_))) => return true,
            Some(Ok(IfEvent::Down(_))) => {}
            Some(Err(why)) => {
                stopped(&why);
                return false;
            }
            None => return false,
        }
    }
}

fn stopped(why: &std::io::Error) {
    elog!("[music] the network watcher stopped; reconnects keep to their backoff: {why}");
}
