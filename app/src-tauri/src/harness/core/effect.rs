//! What a commit leaves to be done with a provider. An effect is made from
//! saved state, runs off the consumer's thread (a CLI can take a minute to
//! answer) and is never replayed after a restart: it lives only in memory.

use std::sync::Arc;

use sqlx::SqlitePool;

use crate::harness::app::{Bus, BusMsg};
use crate::harness::store::RunStart;
use crate::harness::{Harness, Provider, Release, SendOptions};

use super::decide::{Answer, RewindAsk};

pub(in crate::harness) enum Effect {
    /// Send a run that was just written.
    Start(Box<Start>),
    /// Ask the provider to forget a question; its answer comes back on the
    /// bus as [`BusMsg::Answer`] for the command that waits on it.
    Rewind {
        command: String,
        ask: Box<RewindAsk>,
    },
    /// Ask the thread's CLI to stop the turn it has open.
    Interrupt { thread_id: i64 },
    /// Kill what is answering the thread (`Harness::force_stop`).
    ForceStop { thread_id: i64 },
    /// End the thread's session.
    Release { thread_id: i64, reason: Release },
}

#[derive(Clone)]
pub(in crate::harness) struct Start {
    pub thread_id: i64,
    pub provider: Provider,
    pub run: RunStart,
    /// The run's row text: the student's words, or a continuation's line.
    pub text: String,
    pub options: SendOptions,
    /// The chat's model changed with this send.
    pub respawn: bool,
}

/// What an effect works with: the live sessions, the consumer's bus and
/// the database.
#[derive(Clone)]
pub(in crate::harness) struct Runtime {
    pub harness: Arc<Harness>,
    pub bus: Bus,
    pub pools: Pools,
}

/// Where an effect's database is: the app's own, or the one a test made.
#[derive(Clone)]
pub(in crate::harness) enum Pools {
    App,
    #[cfg(test)]
    Given(SqlitePool),
}

impl Pools {
    pub async fn open(&self) -> Result<SqlitePool, String> {
        match self {
            Pools::App => crate::db::store::open_pool().await,
            #[cfg(test)]
            Pools::Given(pool) => Ok(pool.clone()),
        }
    }
}

/// Carry one effect out. A failure is reported on the bus as the events
/// that close what the effect was for; nothing is returned.
pub(in crate::harness) async fn run(rt: &Runtime, effect: Effect) {
    match effect {
        Effect::Start(start) => crate::harness::app::send::start(rt, *start).await,
        Effect::Rewind { command, ask } => {
            let thread_id = ask.thread_id;
            let context = crate::harness::app::rewind::provider(rt, *ask).await;
            let _ = rt.bus.send(BusMsg::Answer {
                thread_id,
                command,
                answer: Answer::Rewound { context },
            });
        }
        Effect::Interrupt { thread_id } => {
            let harness = rt.harness.clone();
            let asked = crate::runtime::blocking::run(move || harness.interrupt(thread_id)).await;
            if let Err(e) = asked {
                elog!("[oculus] harness thread {thread_id}: interrupt: {e}");
            }
        }
        Effect::ForceStop { thread_id } => rt.harness.force_stop(thread_id),
        Effect::Release { thread_id, reason } => rt.harness.release(thread_id, reason),
    }
}
