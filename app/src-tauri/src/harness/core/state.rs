//! What a decider reads: the chat as it is saved, and what the consumer
//! knows of it that no table holds.

use sqlx::SqlitePool;

use crate::harness::store::{self, Question, ThreadRow};

use super::command::{Command, Kind};

/// What only the running app knows about a chat.
#[derive(Clone, Copy, Default)]
pub(in crate::harness) struct Live {
    /// A turn holds the chat: a run that is going out or being answered, a
    /// turn the CLI started itself, or a rewind waiting on the provider.
    pub busy: bool,
}

pub(in crate::harness) struct State {
    /// The chat's row; `None` for a send that starts one.
    pub thread: Option<ThreadRow>,
    pub live: Live,
    /// The question a rewind or an edit names, checked against its row.
    pub question: Option<Question>,
    /// The newest question's anchor, which Claude needs to rewind past it.
    pub newest_anchor: Option<String>,
    /// How many runs wait in the chat's queue.
    pub waiting: i64,
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
    let waiting = match command.thread_id {
        Some(id) => store::queue::waiting(pool, id).await?,
        None => 0,
    };
    let mut state = State {
        thread,
        live,
        question: None,
        newest_anchor: None,
        waiting,
    };
    if let (Some(thread_id), Kind::Rewind { item_id } | Kind::EditResend { item_id, .. }) =
        (command.thread_id, &command.kind)
    {
        state.question = Some(store::user_item(pool, thread_id, *item_id).await?);
        state.newest_anchor = store::newest_anchor(pool, thread_id).await.ok().flatten();
    }
    Ok(state)
}
