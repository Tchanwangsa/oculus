//! What a decider reads: the chat as it is saved, and what the consumer
//! knows of it that no table holds.

use sqlx::SqlitePool;

use crate::harness::manager::policy::{Active, Chat};
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
    /// The native session a rewind would ask to forget the question.
    pub session: Option<String>,
    /// How many runs wait in the chat's queue, and how many of those are
    /// held.
    pub waiting: i64,
    pub held: i64,
    /// The run going out or being answered.
    pub open: Option<OpenRun>,
}

/// What a decider needs of the chat's open run.
#[derive(Clone, Copy)]
pub(in crate::harness) struct OpenRun {
    pub run_id: i64,
    /// Saved and not yet taken by its start step.
    pub starting: bool,
    /// The student already asked it to stop.
    pub stop_requested: bool,
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
            active: self.open.map(|open| Active {
                starting: open.starting,
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
            }),
        ),
        None => ((0, 0), None),
    };
    let mut state = State {
        thread,
        live,
        question: None,
        newest_anchor: None,
        session: None,
        waiting,
        held,
        open,
    };
    if let (Some(thread_id), Kind::Rewind { item_id } | Kind::EditResend { item_id, .. }) =
        (command.thread_id, &command.kind)
    {
        state.question = Some(store::user_item(pool, thread_id, *item_id).await?);
        state.newest_anchor = store::newest_anchor(pool, thread_id).await.ok().flatten();
        if let Some(row) = &state.thread {
            state.session = store::bindings::find(pool, thread_id, row.provider)
                .await?
                .and_then(|binding| binding.native_session);
        }
    }
    Ok(state)
}
