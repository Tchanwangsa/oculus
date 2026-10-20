//! A scripted network for tests: the address list a test sets, and edges it
//! sends, in place of `if-addrs` and `if-watch`.

use super::{Edge, Edges, LocalAddress, Observer, Provider};
use std::net::IpAddr;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::sync::mpsc::{unbounded_channel, UnboundedSender};

#[derive(Default)]
struct List {
    addrs: Mutex<Vec<LocalAddress>>,
    reads: AtomicUsize,
    failing: AtomicBool,
    /// The observer has drained the stream's starting burst.
    watching: AtomicBool,
}

struct Scripted(Arc<List>);

impl Provider for Scripted {
    fn read(&self) -> std::io::Result<Vec<LocalAddress>> {
        self.0.reads.fetch_add(1, Ordering::SeqCst);
        if self.0.failing.load(Ordering::SeqCst) {
            return Err(std::io::Error::other("scripted failure"));
        }
        Ok(self.0.addrs.lock().unwrap().clone())
    }
}

/// The test's side of a fake network.
#[derive(Clone)]
pub struct FakeNetwork {
    list: Arc<List>,
    edges: UnboundedSender<std::io::Result<Edge>>,
}

/// An observer over a fake network holding `initial`.
pub fn fake(initial: Vec<LocalAddress>, debounce: Duration) -> (Observer, FakeNetwork) {
    let list = Arc::new(List::default());
    *list.addrs.lock().unwrap() = initial;
    let (sender, mut receiver) = unbounded_channel();
    let drained = list.clone();
    let open = Box::new(move || {
        let edges = futures_util::stream::poll_fn(move |cx| {
            let next = receiver.poll_recv(cx);
            if next.is_pending() {
                drained.watching.store(true, Ordering::SeqCst);
            }
            next
        });
        Ok(Box::pin(edges) as Edges)
    });
    let observer = Observer::new(Scripted(list.clone()), open, debounce);
    (
        observer,
        FakeNetwork {
            list,
            edges: sender,
        },
    )
}

/// An up address on `interface`.
pub fn addr(ip: &str, interface: &str) -> LocalAddress {
    LocalAddress::new(interface, ip.parse().unwrap(), true, false)
}

impl FakeNetwork {
    /// Replace the list, without an edge.
    pub fn set(&self, addrs: Vec<LocalAddress>) {
        *self.list.addrs.lock().unwrap() = addrs;
    }

    pub fn up(&self, ip: &str) {
        self.edge(Edge::Up(ip.parse::<IpAddr>().unwrap()));
    }

    pub fn down(&self, ip: &str) {
        self.edge(Edge::Down(ip.parse::<IpAddr>().unwrap()));
    }

    pub fn edge(&self, edge: Edge) {
        self.edges.send(Ok(edge)).ok();
    }

    /// The stream fails, as a dead watcher thread does.
    pub fn fail_stream(&self) {
        self.edges.send(Err(std::io::Error::other("scripted"))).ok();
    }

    pub fn fail_reads(&self, failing: bool) {
        self.list.failing.store(failing, Ordering::SeqCst);
    }

    /// Waits until the observer follows the stream: edges sent before this
    /// may be taken for the starting burst.
    pub async fn watching(&self) {
        let deadline = std::time::Instant::now() + Duration::from_secs(2);
        while !self.list.watching.load(Ordering::SeqCst) {
            assert!(
                std::time::Instant::now() < deadline,
                "the observer never started"
            );
            tokio::time::sleep(Duration::from_millis(2)).await;
        }
    }

    pub fn reads(&self) -> usize {
        self.list.reads.load(Ordering::SeqCst)
    }
}
