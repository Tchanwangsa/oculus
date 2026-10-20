//! This machine's addresses, observed once for the whole process. One
//! `if-watch` stream (a second watcher is unsafe on macOS, whose watcher
//! thread cannot be restarted) wakes a re-read of the interface list
//! (`if-addrs`, which names each address's interface). Music subscribes to
//! cut a reconnect backoff short, device sharing to keep its links and Host
//! allow-list current. No OS commands are polled.
//!
//! - `snapshot()` is the list now, with a `generation` that moves only when
//!   the list does. The first call starts the observer.
//! - `subscribe` delivers `Up`/`Down` as the OS reports them and `Changed`
//!   once a burst has been quiet for `DEBOUNCE`.
//! - `refresh()` re-reads at once (app focus, the Share dialog, a Host the
//!   list does not hold).
//!
//! The parts take an injected provider and stream, so tests drive a fake
//! network; the real ones are only in `global()`.

mod address;
mod observer;
mod watch;

#[cfg(test)]
pub mod fake;
#[cfg(test)]
mod tests;

use std::sync::OnceLock;
use std::time::Duration;

pub use address::{Family, Interfaces, LocalAddress, Provider, Snapshot};
pub use observer::{Event, Observer, Subscription};
pub use watch::{Edge, Edges, Open};

/// How long a burst of edges must settle before the list is re-read.
pub const DEBOUNCE: Duration = Duration::from_secs(1);

/// The process's observer over the OS.
pub fn global() -> &'static Observer {
    static GLOBAL: OnceLock<Observer> = OnceLock::new();
    GLOBAL.get_or_init(|| Observer::new(Interfaces, watch::os(), DEBOUNCE))
}

pub fn snapshot() -> Snapshot {
    global().snapshot()
}

pub fn subscribe(callback: impl Fn(&Event) + Send + Sync + 'static) -> Subscription {
    global().subscribe(callback)
}

pub fn refresh() {
    global().refresh();
}
