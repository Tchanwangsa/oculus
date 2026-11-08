//! The Tauri side of the harness: [`HarnessState`] persists every bridge's
//! events (`store::apply`), forwards them to the webview on `harness-event`
//! and releases queued messages as turns close. One module per command
//! family, registered in `lib.rs` by the module that defines it.

pub(in crate::harness) mod consumer;
pub(in crate::harness) mod continuation;
pub mod dictation;
pub mod lifecycle;
pub mod models;
pub(in crate::harness) mod proposal_context;
#[cfg(test)]
mod proposal_context_tests;
pub mod proposal_jobs;
pub mod revision;
mod run_scope;
pub(in crate::harness) mod send;
pub mod setup;
pub mod suggest;
mod thought_clock;
pub mod turns;

pub use lifecycle::{init, reconcile, shutdown, sweep_strays};

use std::sync::{mpsc, Arc, Mutex};

use serde::Serialize;

use crate::harness::{Harness, HarnessEvent, Provider, Queue, Sink};

/// What the webview gets on `harness-event`, plus the row id when the
/// event became a row. Account-scoped events arrive with `threadId` 0,
/// hence the provider. A student send's events also name its run and
/// attempt, and a row it made its weak `entityId`; the timeline reads none yet.
#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub(in crate::harness) struct Envelope {
    pub(in crate::harness) thread_id: i64,
    pub(in crate::harness) provider: Provider,
    pub(in crate::harness) item_id: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(in crate::harness) run_id: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(in crate::harness) attempt_id: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(in crate::harness) entity_id: Option<String>,
    /// Increases with every envelope; the webview ignores one it has seen.
    pub(in crate::harness) sequence: u64,
    pub(in crate::harness) event: HarnessEvent,
}

/// What the one consumer reads, in order; see [`init`]. Events, the hot path,
/// stay unboxed as they were before `Begin` joined them.
#[allow(clippy::large_enum_variant)]
pub(in crate::harness) enum BusMsg {
    /// A bridge's event, tagged with its thread.
    Event(i64, Provider, HarnessEvent),
    /// A send to persist before its provider write. On the consumer, so every
    /// row already queued for the thread is written before the question's.
    Begin {
        intent: crate::harness::store::SendIntent,
        reply: tokio::sync::oneshot::Sender<Result<crate::harness::store::RunStart, String>>,
    },
    /// A claimed send that will not go out (a continuation whose card lost
    /// its claim): the thread's claim passes to whatever waits behind it.
    Release(i64, Provider),
}

type Bus = mpsc::Sender<BusMsg>;

pub struct HarnessState {
    pub harness: Arc<Harness>,
    bus: Bus,
    /// Which threads have a turn open, and what is waiting behind each.
    queue: Arc<Mutex<Queue>>,
}

fn sink_for(bus: &Bus, thread_id: i64, provider: Provider) -> Sink {
    let bus = bus.clone();
    Arc::new(move |ev| {
        let _ = bus.send(BusMsg::Event(thread_id, provider, ev));
    })
}

impl HarnessState {
    fn sink(&self, thread_id: i64, provider: Provider) -> Sink {
        sink_for(&self.bus, thread_id, provider)
    }
}

/// A card was applied or discarded, or its job finished: when the agent gave
/// it a follow-up, the consumer starts that one run (`continuation.rs`).
pub async fn card_settled(
    app: &tauri::AppHandle,
    proposal: &crate::db::proposals::Proposal,
) -> Result<(), String> {
    use tauri::Manager;
    if continuation::follow_up(&proposal.state).is_none() {
        return Ok(());
    }
    let pool = crate::db::store::open_pool().await?;
    let row = crate::harness::store::thread(&pool, proposal.thread_id).await?;
    app.state::<HarnessState>()
        .sink(proposal.thread_id, row.provider)(HarnessEvent::CardSettled {
        proposal: proposal.id.clone(),
    });
    Ok(())
}

/// Proposal writes already committed in Rust; forward the row to every thread view.
pub async fn publish_proposal(
    app: &tauri::AppHandle,
    proposal: crate::db::proposals::Proposal,
) -> Result<(), String> {
    use tauri::Manager;
    let pool = crate::db::store::open_pool().await?;
    let row = crate::harness::store::thread(&pool, proposal.thread_id).await?;
    app.state::<HarnessState>()
        .sink(proposal.thread_id, row.provider)(HarnessEvent::Proposal { proposal });
    Ok(())
}
