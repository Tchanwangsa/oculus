//! The OS's address-change stream: `if-watch` (SCDynamicStore on macOS, the
//! IP helper on Windows, netlink on Linux). Opened once per process: macOS's
//! watcher thread outlives a dropped watcher, so it is never reopened.

use futures_util::{Stream, StreamExt};
use if_watch::tokio::IfWatcher;
use if_watch::IfEvent;
use std::net::IpAddr;
use std::pin::Pin;

/// An address appearing or going away.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Edge {
    Up(IpAddr),
    Down(IpAddr),
}

pub type Edges = Pin<Box<dyn Stream<Item = std::io::Result<Edge>> + Send>>;

/// Opens the stream; called once, inside the observer's task. Its first
/// items, available at once, are the addresses already there.
pub type Open = Box<dyn FnOnce() -> std::io::Result<Edges> + Send>;

pub fn os() -> Open {
    Box::new(|| {
        let watcher = IfWatcher::new()?;
        let edges = watcher.map(|event| {
            event.map(|event| match event {
                IfEvent::Up(net) => Edge::Up(net.addr()),
                IfEvent::Down(net) => Edge::Down(net.addr()),
            })
        });
        Ok(Box::pin(edges) as Edges)
    })
}
