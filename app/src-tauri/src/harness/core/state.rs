//! What a decider reads: the chat as it is saved, and what the consumer
//! knows of it that no table holds.

use sqlx::SqlitePool;

use crate::harness::store::{self, Question, ThreadRow};

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
}

pub(in crate::harness) struct State {
    /// The chat's row; `None` for a send that starts one.
    pub thread: Option<ThreadRow>,
    pub live: Live,
    /// The question a rewind or an edit names, checked against its row.
    pub question: Option<Question>,
    /// The newest question's anchor, which Claude needs to rewind past it.
    pub newest_anchor: Option<String>,
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
    /// The student already asked it to stop.
    pub stop_requested: bool,
}

impl State {
    /// A turn has the chat: a message sent now waits, and nothing may
    /// rewrite the chat's rows.
    pub fn busy(&self) -> bool {
        self.open.is_some() || self.live.held
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
        waiting,
        held,
        open,
    };
    if let (Some(thread_id), Kind::Rewind { item_id } | Kind::EditResend { item_id, .. }) =
        (command.thread_id, &command.kind)
    {
        state.question = Some(store::user_item(pool, thread_id, *item_id).await?);
        state.newest_anchor = store::newest_anchor(pool, thread_id).await.ok().flatten();
    }
    Ok(state)
}
