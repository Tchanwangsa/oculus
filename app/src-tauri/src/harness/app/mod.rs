//! The Tauri side of the harness: [`HarnessState`] persists every bridge's
//! events (`store::apply`), forwards them to the webview on `harness-event`
//! and releases queued messages as turns close. One module per command
//! family, registered in `lib.rs` by the module that defines it.

pub mod lifecycle;
pub mod models;
mod send;
pub mod setup;
pub mod suggest;
pub mod turns;

pub use lifecycle::{init, reconcile, shutdown, sweep_strays};

use std::sync::{mpsc, Arc, Mutex};

use serde::Serialize;

use crate::harness::{Harness, HarnessEvent, Provider, Queue, Sink};

/// What the webview gets on `harness-event`, plus the row id when the
/// event became a row. Account-scoped events arrive with `threadId` 0,
/// hence the provider.
#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct Envelope {
    thread_id: i64,
    provider: Provider,
    item_id: Option<i64>,
    event: HarnessEvent,
}

/// Where every bridge's events go, tagged with the thread they belong
/// to. One consumer reads it; see [`init`].
type Bus = mpsc::Sender<(i64, Provider, HarnessEvent)>;

pub struct HarnessState {
    pub harness: Arc<Harness>,
    bus: Bus,
    /// Which threads have a turn open, and what is waiting behind each.
    queue: Arc<Mutex<Queue>>,
}

fn sink_for(bus: &Bus, thread_id: i64, provider: Provider) -> Sink {
    let bus = bus.clone();
    Arc::new(move |ev| {
        let _ = bus.send((thread_id, provider, ev));
    })
}

impl HarnessState {
    fn sink(&self, thread_id: i64, provider: Provider) -> Sink {
        sink_for(&self.bus, thread_id, provider)
    }
}
