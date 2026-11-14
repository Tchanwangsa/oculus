//! A thread's turns as Tauri commands: send, edit and resend, rewind, the
//! queue behind a running turn, interrupt and delete. Each builds a
//! [`Command`], hands it to the consumer and returns what was committed
//! (`harness/core/`); none writes or calls a provider itself.

use tauri::State;

use crate::harness::core::{load, Command, Kind, Outcome};
use crate::harness::manager::validate_effort;
use crate::harness::store;
use crate::harness::{HarnessEvent, Provider, SendOptions};
use crate::runtime::blocking::run as blocking;

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
/// a message sent mid-turn waits in the [`Queue`](crate::harness::Queue).
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

/// A thread as a reload reads it: rows, runs and the revision that covers
/// them in one read, then what waits behind its turn (the queue is only in
/// memory).
#[tauri::command]
pub async fn harness_load(
    state: State<'_, HarnessState>,
    thread_id: i64,
) -> Result<load::Snapshot, String> {
    let pool = crate::db::store::open_pool().await?;
    let mut snapshot = load::read(&pool, thread_id).await?;
    snapshot.queued = state.queue.lock().unwrap().list(thread_id);
    Ok(snapshot)
}

/// Drop one message that has not gone out yet.
#[tauri::command]
pub async fn harness_unqueue(
    state: State<'_, HarnessState>,
    thread_id: i64,
    queue_id: String,
) -> Result<(), String> {
    if state.queue.lock().unwrap().remove(thread_id, &queue_id) {
        let pool = crate::db::store::open_pool().await?;
        let row = store::thread(&pool, thread_id).await?;
        state.sink(thread_id, row.provider)(HarnessEvent::Unqueued { id: queue_id });
    }
    Ok(())
}

/// Rewrite one that has not gone out yet; it comes back as a `queued`
/// event under the same id, so the webview replaces it.
#[tauri::command]
pub async fn harness_edit_queued(
    state: State<'_, HarnessState>,
    thread_id: i64,
    queue_id: String,
    text: String,
) -> Result<(), String> {
    let edited = state
        .queue
        .lock()
        .unwrap()
        .edit(thread_id, &queue_id, &text);
    if let Some(msg) = edited {
        let pool = crate::db::store::open_pool().await?;
        let row = store::thread(&pool, thread_id).await?;
        state.sink(thread_id, row.provider)(HarnessEvent::Queued {
            id: msg.id,
            text: msg.text,
        });
    }
    Ok(())
}

/// Stop the running turn and drop whatever was waiting behind it,
/// handing the dropped texts back to the composer. A card's continuation
/// waiting there is cancelled too, and its card says so.
#[tauri::command]
pub async fn harness_interrupt(
    state: State<'_, HarnessState>,
    thread_id: i64,
) -> Result<Vec<String>, String> {
    let cleared = state.queue.lock().unwrap().clear(thread_id);
    if !cleared.messages.is_empty() || !cleared.continuations.is_empty() {
        let pool = crate::db::store::open_pool().await?;
        let row = store::thread(&pool, thread_id).await?;
        let sink = state.sink(thread_id, row.provider);
        for m in &cleared.messages {
            sink(HarnessEvent::Unqueued { id: m.id.clone() });
        }
        for card in &cleared.continuations {
            if super::continuation::cancel(&pool, card, super::continuation::STOPPED).await? {
                let proposal = crate::db::proposals::get(&pool, card).await?;
                sink(HarnessEvent::Proposal { proposal });
            }
        }
    }
    let h = state.harness.clone();
    blocking(move || h.interrupt(thread_id)).await?;
    Ok(cleared.messages.into_iter().map(|m| m.text).collect())
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
