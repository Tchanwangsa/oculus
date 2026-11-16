//! What a decider reads: the chat as it is saved, and what the consumer
//! knows of it that no table holds.

use sqlx::SqlitePool;

use crate::harness::manager::policy::{Active, Chat};
use crate::harness::requests::{self, RequestView};
use crate::harness::store::bindings::Holder;
use crate::harness::store::{self, Question, ThreadRow};
use crate::harness::Provider;

use super::command::{Command, Kind};

/// What only the running app knows about a chat.
#[derive(Clone, Copy, Default)]
pub(in crate::harness) struct Live {
    /// Something no row shows holds the chat: a turn the CLI started
    /// itself, a rewind waiting on the provider, or a settle that has not
    /// landed yet.
    pub held: bool,
    /// The CLI is in a turn it started itself, which no run stands for.
    pub own_turn: bool,
    /// The chat's live session has a background task running, which a
    /// switch to another provider would kill.
    pub background: bool,
    /// Which bridges can take over a chat another provider started.
    pub takeover: Takeover,
    /// The request a `Respond` names still has its session's route: the
    /// session that raised it is the live one.
    pub route: bool,
    /// Which bridges take a message into a running turn, natively or by
    /// restarting it.
    pub steering: Steering,
    /// The open run's start step reported that its provider has the turn:
    /// before that there is no turn to write into.
    pub accepted: bool,
}

/// The bridges whose `active_steering` and `steer_by_restart` capabilities
/// are true, as the running app holds them.
#[derive(Clone, Copy, Default, PartialEq, Debug)]
pub(in crate::harness) struct Steering {
    native: Takeover,
    restart: Takeover,
}

impl Steering {
    pub fn read(
        caps: impl Fn(Provider) -> crate::harness::providers::capabilities::Capabilities,
    ) -> Self {
        Steering {
            native: Takeover::read(|p| caps(p).active_steering),
            restart: Takeover::read(|p| caps(p).steer_by_restart),
        }
    }

    /// `provider`'s flags, over the bridge's own capabilities.
    pub fn of(self, provider: Provider) -> crate::harness::providers::capabilities::Capabilities {
        crate::harness::providers::capabilities::Capabilities {
            active_steering: self.native.allows(provider),
            steer_by_restart: self.restart.allows(provider),
            ..crate::harness::providers::capabilities::of(provider)
        }
    }
}

/// The bridges whose `provider_handoff` capability is true, as the running
/// app holds them (`Harness::capabilities`).
#[derive(Clone, Copy, Default, PartialEq, Debug)]
pub(in crate::harness) struct Takeover(u8);

impl Takeover {
    fn bit(provider: Provider) -> u8 {
        match provider {
            Provider::Claude => 1,
            Provider::Codex => 2,
            Provider::Opencode => 4,
            Provider::Antigravity => 8,
        }
    }

    pub fn read(handoff: impl Fn(Provider) -> bool) -> Self {
        let providers = crate::harness::discover::PROVIDERS;
        Takeover(
            providers
                .into_iter()
                .filter(|provider| handoff(*provider))
                .fold(0, |bits, provider| bits | Self::bit(provider)),
        )
    }

    pub fn allows(self, provider: Provider) -> bool {
        self.0 & Self::bit(provider) != 0
    }
}

pub(in crate::harness) struct State {
    /// The chat's row; `None` for a send that starts one.
    pub thread: Option<ThreadRow>,
    pub live: Live,
    /// The question a rewind or an edit names, checked against its row.
    pub question: Option<Question>,
    /// The newest question's anchor, which Claude needs to rewind past it.
    pub newest_anchor: Option<String>,
    /// The provider session that holds that question, which a rewind asks
    /// to forget it: its own run's, whatever the chat has switched to since.
    pub holder: Option<Holder>,
    /// How many runs wait in the chat's queue, and how many of those are
    /// held.
    pub waiting: i64,
    pub held: i64,
    /// The run going out or being answered.
    pub open: Option<OpenRun>,
    /// The request a `Respond` names, if it is this chat's.
    pub request: Option<RequestView>,
    /// How many of the chat's requests wait for the student.
    pub pending: i64,
    /// Of those, the questions: while one waits, typed text answers it.
    pub questions: i64,
    /// The approvals the open run waits on, each with the option that
    /// declines it: a steer declines them on its way in.
    pub approvals: Vec<(i64, String)>,
    /// The text of the waiting message a `SteerQueued` names, while it
    /// still waits.
    pub queued: Option<String>,
}

/// What a decider needs of the chat's open run.
#[derive(Clone, Copy)]
pub(in crate::harness) struct OpenRun {
    pub run_id: i64,
    /// Saved and not yet taken by its start step.
    pub starting: bool,
    /// The student already asked it to stop.
    pub stop_requested: bool,
    /// The bridge it runs on, which a steer is written to.
    pub provider: Provider,
    /// Its active attempt and binding, which a steer's row joins.
    pub attempt_id: i64,
    pub binding_id: i64,
}

impl State {
    /// A turn has the chat: a message sent now waits, and nothing may
    /// rewrite the chat's rows.
    pub fn busy(&self) -> bool {
        self.open.is_some() || self.live.held
    }

    /// The chat as the delivery policy reads it (`manager/policy.rs`).
    pub fn chat(&self) -> Chat {
        Chat {
            // A run its provider has not taken yet has no turn to write into.
            active: self.open.map(|open| Active {
                starting: open.starting || !self.live.accepted,
                stopping: open.stop_requested,
            }),
            held: self.live.held,
            waiting: self.waiting > 0,
        }
    }
}

/// Read everything `command`'s decider may ask. An `Err` is the command's
/// refusal: a chat or a question that is not there.
pub(in crate::harness) async fn read(
    pool: &SqlitePool,
    command: &Command,
    live: Live,
) -> Result<State, String> {
    let thread = match command.thread_id {
        Some(id) => Some(store::thread(pool, id).await?),
        None => None,
    };
    let ((waiting, held), open) = match command.thread_id {
        Some(id) => (
            store::queue::waiting(pool, id).await?,
            store::queue::open(pool, id).await?.map(|open| OpenRun {
                run_id: open.run.run_id,
                starting: open.starting,
                stop_requested: open.stop_requested,
                provider: open.provider,
                attempt_id: open.run.attempt_id,
                binding_id: open.run.binding_id,
            }),
        ),
        None => ((0, 0), None),
    };
    let pending = match command.thread_id {
        Some(id) => requests::store::pending(pool, id).await?,
        None => 0,
    };
    let (questions, approvals) = match (command.thread_id, &command.kind, open) {
        (Some(id), Kind::Send { .. } | Kind::SteerQueued { .. }, Some(open)) => {
            waiting_on(pool, id, open.run_id).await?
        }
        _ => (0, Vec::new()),
    };
    let queued = match (command.thread_id, &command.kind) {
        (Some(id), Kind::SteerQueued { run_id }) => sqlx::query_scalar(
            "SELECT queued_text FROM harness_runs
              WHERE id = ?1 AND thread_id = ?2 AND execution_status = 'queued' AND origin = 'student'",
        )
        .bind(run_id)
        .bind(id)
        .fetch_optional(pool)
        .await
        .map_err(|e| e.to_string())?
        .flatten(),
        _ => None,
    };
    let request = match (command.thread_id, &command.kind) {
        (Some(thread_id), Kind::Respond { request_id, .. }) => {
            let mut conn = pool.acquire().await.map_err(|e| e.to_string())?;
            requests::store::get(&mut conn, *request_id)
                .await?
                .filter(|r| r.thread_id == thread_id)
        }
        _ => None,
    };
    let mut state = State {
        thread,
        live,
        question: None,
        newest_anchor: None,
        holder: None,
        waiting,
        held,
        open,
        request,
        pending,
        questions,
        approvals,
        queued,
    };
    if let (Some(thread_id), Kind::Rewind { item_id } | Kind::EditResend { item_id, .. }) =
        (command.thread_id, &command.kind)
    {
        state.question = Some(store::user_item(pool, thread_id, *item_id).await?);
        state.newest_anchor = store::newest_anchor(pool, thread_id).await.ok().flatten();
        state.holder = store::bindings::holder(pool, thread_id, *item_id).await?;
    }
    Ok(state)
}

/// What the open run waits on the student for: how many questions, and the
/// approvals with the option that declines each.
async fn waiting_on(
    pool: &SqlitePool,
    thread_id: i64,
    run_id: i64,
) -> Result<(i64, Vec<(i64, String)>), String> {
    let mut conn = pool.acquire().await.map_err(|e| e.to_string())?;
    let pending: Vec<RequestView> = requests::store::list(&mut conn, thread_id)
        .await?
        .into_iter()
        .filter(|r| r.status == requests::Status::Pending)
        .collect();
    let questions = pending.iter().filter(|r| r.kind == "question").count() as i64;
    let approvals = pending
        .iter()
        .filter(|r| r.kind != "question" && r.responder == "live" && r.run_id == Some(run_id))
        .filter_map(|r| {
            let decline = r
                .options
                .iter()
                .find(|o| o.decision == requests::Decision::Decline)?;
            Some((r.id, decline.id.clone()))
        })
        .collect();
    Ok((questions, approvals))
}
