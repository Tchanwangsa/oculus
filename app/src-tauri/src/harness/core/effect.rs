//! What a commit leaves to be done with a provider. An effect is made from
//! saved state, runs off the consumer's thread (a CLI can take a minute to
//! answer) and is never replayed after a restart: it lives only in memory.

use std::sync::Arc;

use sqlx::SqlitePool;

use crate::harness::app::{Bus, BusMsg};
use crate::harness::requests::{LiveRoute, RequestOption};
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
    /// Write a steer into the thread's running turn; what the provider
    /// said comes back on the bus as [`BusMsg::Steered`].
    Steer(Box<SteerWrite>),
    /// Ask the thread's CLI to stop the turn it has open.
    Interrupt { thread_id: i64 },
    /// Kill what is answering the thread (`Harness::force_stop`).
    ForceStop { thread_id: i64 },
    /// End the thread's session.
    Release { thread_id: i64, reason: Release },
    /// End the thread's session if it is `provider`'s: a rewind took rows
    /// that session had seen.
    Forget { thread_id: i64, provider: Provider },
    /// Write the student's answer to the callback it was saved for; what
    /// the write proved comes back as [`BusMsg::Replied`].
    Reply(Box<Reply>),
    /// Tell these callbacks no without asking anyone: requests the app
    /// gave up or never showed.
    Withdraw { routes: Vec<LiveRoute> },
}

pub(in crate::harness) struct Reply {
    pub thread_id: i64,
    pub provider: Provider,
    pub request_id: i64,
    pub route: LiveRoute,
    pub option: RequestOption,
    pub answers: Vec<crate::harness::requests::QuestionAnswer>,
}

/// A steer as its effect writes it.
#[derive(Clone)]
pub(in crate::harness) struct SteerWrite {
    pub thread_id: i64,
    pub provider: Provider,
    pub written: crate::harness::store::steer::Written,
    pub text: String,
    pub client: String,
}

#[derive(Clone)]
pub(in crate::harness) struct Start {
    pub thread_id: i64,
    pub provider: Provider,
    pub run: RunStart,
    /// The run's row text: the student's words, or a continuation's line.
    pub text: String,
    pub options: SendOptions,
    /// The model and reasoning level the run was written with.
    pub model: Option<String>,
    pub effort: Option<String>,
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
        Effect::Steer(steer) => crate::harness::app::steer::write(rt, *steer).await,
        Effect::Interrupt { thread_id } => {
            let harness = rt.harness.clone();
            let asked = crate::runtime::blocking::run(move || harness.interrupt(thread_id)).await;
            if let Err(e) = asked {
                elog!("[oculus] harness thread {thread_id}: interrupt: {e}");
            }
        }
        Effect::ForceStop { thread_id } => rt.harness.force_stop(thread_id),
        Effect::Release { thread_id, reason } => rt.harness.release(thread_id, reason),
        Effect::Forget {
            thread_id,
            provider,
        } => rt.harness.release_provider(thread_id, provider),
        Effect::Reply(reply) => {
            let Reply {
                thread_id,
                provider,
                request_id,
                route,
                option,
                answers,
            } = *reply;
            let written =
                crate::runtime::blocking::run(move || Ok(route.0.reply_answers(&option, &answers)));
            let reply = written
                .await
                .unwrap_or_else(crate::harness::requests::Reply::Uncertain);
            let _ = rt.bus.send(BusMsg::Replied {
                thread_id,
                provider,
                request_id,
                reply,
            });
        }
        Effect::Withdraw { routes } => {
            let withdrawn = crate::runtime::blocking::run(move || {
                routes.iter().for_each(|route| route.0.withdraw());
                Ok(())
            });
            let _: Result<(), String> = withdrawn.await;
        }
    }
}
