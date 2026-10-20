//! In-process fan-out of authority events. Desktop panes receive them as Tauri
//! events (`commands.rs`); a network transport subscribes to the same list.
//! Callbacks run while the session's lock is held, which keeps each file's
//! events in revision order: they must return quickly and must not call back
//! into the authority.

use serde::Serialize;
use serde_json::Value;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, RwLock};

/// An accepted change to a shared canvas's content.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Change {
    pub file_id: i64,
    pub epoch: String,
    pub revision: u64,
    /// The committing client, or `None` when the disk version was adopted.
    pub client_id: Option<String>,
    #[serde(serialize_with = "shared")]
    pub transaction: Arc<Value>,
}

fn shared<S: serde::Serializer>(value: &Arc<Value>, serializer: S) -> Result<S::Ok, S::Error> {
    value.as_ref().serialize(serializer)
}

/// Where a session's lifecycle stands.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Phase {
    Live,
    /// The file changed outside the authority; commits wait for a resolution.
    Paused,
    Ended,
}

/// A lifecycle change: paused by an external edit, resumed, or ended.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct State {
    pub file_id: i64,
    pub epoch: String,
    pub phase: Phase,
}

#[derive(Debug, Clone)]
pub enum Event {
    Change(Change),
    State(State),
}

type Callback = Arc<dyn Fn(&Event) + Send + Sync>;

#[derive(Default)]
pub struct Subscribers {
    list: RwLock<Vec<(u64, Callback)>>,
    next: AtomicU64,
}

impl Subscribers {
    /// Register `callback` for the life of the process; returns its id.
    pub fn subscribe(&self, callback: impl Fn(&Event) + Send + Sync + 'static) -> u64 {
        let id = self.next.fetch_add(1, Ordering::Relaxed) + 1;
        self.list
            .write()
            .unwrap_or_else(|e| e.into_inner())
            .push((id, Arc::new(callback)));
        id
    }

    pub fn emit(&self, event: &Event) {
        let list = self.list.read().unwrap_or_else(|e| e.into_inner()).clone();
        for (_, callback) in list {
            callback(event);
        }
    }
}
