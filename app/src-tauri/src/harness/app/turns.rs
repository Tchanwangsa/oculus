//! A thread's turns as Tauri commands: send, edit and resend, rewind, the
//! queue behind a running turn, stop and delete. Each builds a
//! [`Command`], hands it to the consumer and returns what was committed
//! (`harness/core/`); none writes or calls a provider itself.

use tauri::State;

use crate::harness::core::{load, Command, Kind, Outcome};
use crate::harness::manager::validate_effort;
use crate::harness::{Provider, SendOptions};

use super::{BusMsg, HarnessState};

impl HarnessState {
    /// Put a command on the consumer's bus once startup's repair is over,
    /// and wait for its commit.
    pub(in crate::harness) async fn command(&self, command: Command) -> Result<Outcome, String> {
        self.ready().await?;
        let (reply, answer) = tokio::sync::oneshot::channel();
        self.bus
            .send(BusMsg::Command { command, reply })
            .map_err(|_| "the harness consumer has stopped".to_string())?;
        answer
            .await
            .map_err(|_| "the harness consumer has stopped".to_string())?
    }
}

/// Send a message; creates the thread when `thread_id` is null. Returns
/// the thread id once the message is saved: its run goes out afterwards, and
/// a message sent mid-turn waits as a queued run (`store/queue.rs`).
/// `command_id` is the webview's id for this send; the same one again
/// starts nothing and returns the same thread.
#[tauri::command]
pub async fn harness_send(
    state: State<'_, HarnessState>,
    command_id: String,
    thread_id: Option<i64>,
    provider: String,
    text: String,
    options: Option<SendOptions>,
) -> Result<i64, String> {
    let provider =
        Provider::parse(&provider).ok_or_else(|| format!("unknown provider {provider}"))?;
    let mut options = options.unwrap_or_default();
    options.reasoning_effort = validate_effort(options.reasoning_effort)?;
    options.scope_new_thread(thread_id)?;
    if let (None, Some(video)) = (thread_id, &options.studio) {
        crate::studio::videos::video_in(&crate::studio::videos::videos_dir(), &video.slug)?;
    }
    let kind = Kind::Send {
        provider,
        text,
        options,
    };
    let sent = state.command(Command {
        id: command_id,
        thread_id,
        kind,
    });
    Ok(sent.await?.thread_id)
}

/// Ask a question again, differently: rewind the thread to that question
/// (rows and, where it can, the agent's context) and send the new text.
/// A provider that cannot rewind is not fatal — the rows still go, and
/// the `Rewound` event says the agent kept the original.
#[tauri::command]
pub async fn harness_edit_resend(
    state: State<'_, HarnessState>,
    command_id: String,
    thread_id: i64,
    item_id: i64,
    text: String,
    options: Option<SendOptions>,
) -> Result<(), String> {
    let mut options = options.unwrap_or_default();
    options.reasoning_effort = validate_effort(options.reasoning_effort)?;
    let kind = Kind::EditResend {
        item_id,
        text,
        options,
    };
    let edited = state.command(Command {
        id: command_id,
        thread_id: Some(thread_id),
        kind,
    });
    edited.await.map(|_| ())
}

/// Take the thread back to just before a question and hand the question
/// back for the composer. Unlike [`harness_edit_resend`], nothing is sent.
#[tauri::command]
pub async fn harness_rewind(
    state: State<'_, HarnessState>,
    command_id: String,
    thread_id: i64,
    item_id: i64,
) -> Result<String, String> {
    let rewound = state.command(Command {
        id: command_id,
        thread_id: Some(thread_id),
        kind: Kind::Rewind { item_id },
    });
    Ok(rewound.await?.text.unwrap_or_default())
}

/// A thread as a reload reads it: rows, runs, what waits in its queue and
/// the revision that covers them, in one read.
#[tauri::command]
pub async fn harness_load(thread_id: i64) -> Result<load::Snapshot, String> {
    let pool = crate::db::store::open_pool().await?;
    load::read(&pool, thread_id).await
}

/// A waiting message's id, as the webview holds it: its run's.
fn queued_run(queue_id: &str) -> Result<i64, String> {
    queue_id
        .parse()
        .map_err(|_| format!("no waiting message {queue_id}"))
}

/// Drop one message that has not gone out yet.
#[tauri::command]
pub async fn harness_unqueue(
    state: State<'_, HarnessState>,
    command_id: String,
    thread_id: i64,
    queue_id: String,
) -> Result<(), String> {
    let run_id = queued_run(&queue_id)?;
    let removed = state.command(Command {
        id: command_id,
        thread_id: Some(thread_id),
        kind: Kind::RemoveQueued { run_id },
    });
    removed.await.map(|_| ())
}

/// Rewrite one that has not gone out yet; the queue is told again, whole.
#[tauri::command]
pub async fn harness_edit_queued(
    state: State<'_, HarnessState>,
    command_id: String,
    thread_id: i64,
    queue_id: String,
    text: String,
) -> Result<(), String> {
    let run_id = queued_run(&queue_id)?;
    let edited = state.command(Command {
        id: command_id,
        thread_id: Some(thread_id),
        kind: Kind::EditQueued { run_id, text },
    });
    edited.await.map(|_| ())
}

/// Put the waiting messages in `order`, first to go out first.
#[tauri::command]
pub async fn harness_reorder_queued(
    state: State<'_, HarnessState>,
    command_id: String,
    thread_id: i64,
    order: Vec<String>,
) -> Result<(), String> {
    let order = order
        .iter()
        .map(|id| queued_run(id))
        .collect::<Result<Vec<_>, _>>()?;
    let moved = state.command(Command {
        id: command_id,
        thread_id: Some(thread_id),
        kind: Kind::ReorderQueued { order },
    });
    moved.await.map(|_| ())
}

/// Stop the running turn and drop whatever was waiting behind it,
/// handing the dropped texts back to the composer. A card's continuation
/// waiting there is cancelled too, and its card says so.
#[tauri::command]
pub async fn harness_interrupt(
    state: State<'_, HarnessState>,
    command_id: String,
    thread_id: i64,
) -> Result<Vec<String>, String> {
    let stopped = state.command(Command {
        id: command_id,
        thread_id: Some(thread_id),
        kind: Kind::Stop,
    });
    Ok(stopped.await?.dropped)
}

#[tauri::command]
pub async fn harness_delete_thread(
    state: State<'_, HarnessState>,
    command_id: String,
    thread_id: i64,
) -> Result<(), String> {
    let deleted = state.command(Command {
        id: command_id,
        thread_id: Some(thread_id),
        kind: Kind::DeleteThread,
    });
    deleted.await.map(|_| ())
}
