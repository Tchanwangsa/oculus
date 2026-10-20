//! The observer: a snapshot of the address list, kept current from the OS's
//! change stream, and the subscribers told about it. Edges (`Up`, `Down`)
//! reach subscribers as they arrive; `Changed` comes once a burst has been
//! quiet for the debounce window and the re-read list differs. `Changed`
//! may also come from `refresh` on the caller's thread, so a subscriber
//! orders snapshots by `generation`, not by arrival.

use super::address::{normalize, LocalAddress, Provider, Snapshot};
use super::watch::{Edge, Edges, Open};
use futures_util::{FutureExt, StreamExt};
use std::net::IpAddr;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, Weak};
use std::time::Duration;

#[derive(Debug, Clone)]
pub enum Event {
    /// The list changed; carries the new snapshot.
    Changed(Snapshot),
    Up(IpAddr),
    Down(IpAddr),
}

type Callback = Arc<dyn Fn(&Event) + Send + Sync>;

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(|e| e.into_inner())
}

struct Inner {
    provider: Box<dyn Provider>,
    /// Taken by the first start; the stream is never reopened.
    open: Mutex<Option<Open>>,
    debounce: Duration,
    /// `None` until started.
    current: Mutex<Option<Snapshot>>,
    subscribers: Mutex<Vec<(u64, Callback)>>,
    next_id: AtomicU64,
    /// A read failed and was logged; the next failure stays quiet.
    unreadable: AtomicBool,
}

/// A handle on one observer; cheap to clone.
#[derive(Clone)]
pub struct Observer {
    inner: Arc<Inner>,
}

/// Unsubscribes when dropped.
#[must_use = "dropping a subscription unsubscribes"]
pub struct Subscription {
    inner: Weak<Inner>,
    id: u64,
}

impl Drop for Subscription {
    fn drop(&mut self) {
        if let Some(inner) = self.inner.upgrade() {
            lock(&inner.subscribers).retain(|(id, _)| *id != self.id);
        }
    }
}

impl Observer {
    /// An observer over `provider`, woken by the stream `open` makes. Nothing
    /// is read or opened until the first `snapshot` or `subscribe`.
    pub fn new(provider: impl Provider, open: Open, debounce: Duration) -> Self {
        Self {
            inner: Arc::new(Inner {
                provider: Box::new(provider),
                open: Mutex::new(Some(open)),
                debounce,
                current: Mutex::new(None),
                subscribers: Mutex::new(Vec::new()),
                next_id: AtomicU64::new(1),
                unreadable: AtomicBool::new(false),
            }),
        }
    }

    /// The current list, starting the observer if it has not started.
    pub fn snapshot(&self) -> Snapshot {
        self.inner.start()
    }

    /// Calls `callback` with every event from now on, starting the observer
    /// if it has not started. It runs on the observer's task or on a
    /// `refresh` caller's thread, and must not block.
    pub fn subscribe(&self, callback: impl Fn(&Event) + Send + Sync + 'static) -> Subscription {
        self.inner.start();
        let id = self.inner.next_id.fetch_add(1, Ordering::Relaxed);
        lock(&self.inner.subscribers).push((id, Arc::new(callback)));
        Subscription {
            inner: Arc::downgrade(&self.inner),
            id,
        }
    }

    /// Re-read the list now and publish `Changed` if it moved: for an app
    /// coming back to the front, a dialog opening, or a Host the list does
    /// not hold. Before the observer has started it does nothing; the first
    /// snapshot reads fresh anyway.
    pub fn refresh(&self) {
        self.inner.settle();
    }
}

impl Inner {
    fn start(self: &Arc<Self>) -> Snapshot {
        let mut current = lock(&self.current);
        if let Some(snapshot) = current.as_ref() {
            return snapshot.clone();
        }
        let snapshot = Snapshot {
            generation: 1,
            addrs: normalize(self.read().unwrap_or_default()),
        };
        *current = Some(snapshot.clone());
        drop(current);
        if let Some(open) = lock(&self.open).take() {
            tauri::async_runtime::spawn(watch(self.clone(), open));
        }
        snapshot
    }

    fn read(&self) -> Option<Vec<LocalAddress>> {
        match self.provider.read() {
            Ok(addrs) => {
                self.unreadable.store(false, Ordering::Relaxed);
                Some(addrs)
            }
            Err(error) => {
                if !self.unreadable.swap(true, Ordering::Relaxed) {
                    elog!("[network] interface addresses unreadable: {error}");
                }
                None
            }
        }
    }

    /// Re-read; a list that moved gets a new generation and a `Changed`. An
    /// unreadable list keeps the last one.
    fn settle(&self) {
        let changed = {
            let mut current = lock(&self.current);
            let Some(snapshot) = current.as_mut() else {
                return;
            };
            let Some(addrs) = self.read() else {
                return;
            };
            let addrs = normalize(addrs);
            if addrs == snapshot.addrs {
                return;
            }
            snapshot.generation += 1;
            snapshot.addrs = addrs;
            snapshot.clone()
        };
        self.dispatch(&Event::Changed(changed));
    }

    fn dispatch(&self, event: &Event) {
        let subscribers: Vec<Callback> = lock(&self.subscribers)
            .iter()
            .map(|(_, callback)| callback.clone())
            .collect();
        for callback in subscribers {
            callback(event);
        }
    }

    fn edge(&self, edge: Edge) {
        self.dispatch(&match edge {
            Edge::Up(ip) => Event::Up(ip),
            Edge::Down(ip) => Event::Down(ip),
        });
    }
}

/// Follows the stream for the life of the process. A stream error ends it,
/// logged once; the list then moves only on `refresh`, and the stream is
/// never reopened (macOS cannot restart its watcher thread).
async fn watch(inner: Arc<Inner>, open: Open) {
    let mut edges: Edges = match open() {
        Ok(edges) => edges,
        Err(why) => return stopped(&why),
    };
    // The addresses there at the start arrive as events at once.
    loop {
        match edges.next().now_or_never() {
            Some(Some(Ok(_))) => {}
            Some(Some(Err(why))) => return stopped(&why),
            Some(None) => return,
            None => break,
        }
    }
    // Anything that moved between the first read and the stream opening.
    inner.settle();
    loop {
        match edges.next().await {
            Some(Ok(edge)) => inner.edge(edge),
            Some(Err(why)) => return stopped(&why),
            None => return,
        }
        // Interfaces flap as a network joins: one re-read per burst, at the
        // end of the window that its first edge opened.
        let quiet = tokio::time::sleep(inner.debounce);
        tokio::pin!(quiet);
        let ended = loop {
            tokio::select! {
                _ = &mut quiet => break None,
                next = edges.next() => match next {
                    Some(Ok(edge)) => inner.edge(edge),
                    Some(Err(why)) => break Some(Some(why)),
                    None => break Some(None),
                },
            }
        };
        inner.settle();
        if let Some(end) = ended {
            if let Some(why) = end {
                stopped(&why);
            }
            return;
        }
    }
}

fn stopped(why: &std::io::Error) {
    elog!("[network] the address watcher stopped; addresses now re-read only on refresh: {why}");
}
