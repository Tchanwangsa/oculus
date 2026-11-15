//! A thread's turns as Tauri commands: send, edit and resend, rewind, the
//! queue behind a running turn, stop and delete. Each builds a
//! [`Command`], hands it to the consumer and returns what was committed
//! (`harness/core/`); none writes or calls a provider itself.

use tauri::State;

use crate::harness::core::{load, Command, Kind, Outcome};
use crate::harness::manager::policy::Mode;
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
/// starts nothing and returns the same thread. `mode` is how the message
/// should reach a chat that is mid-turn (`manager/policy.rs`); the
/// composer's plain send names none.
#[tauri::command]
pub async fn harness_send(
    state: State<'_, HarnessState>,
    command_id: String,
    thread_id: Option<i64>,
    provider: String,
    text: String,
    options: Option<SendOptions>,
    mode: Option<Mode>,
) -> Result<i64, String> {
    let provider = parse_provider(&provider)?;
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
        mode: mode.unwrap_or_default(),
    };
    let sent = state.command(Command {
        id: command_id,
        thread_id,
        kind,
    });
    Ok(sent.await?.thread_id)
}

fn parse_provider(provider: &str) -> Result<Provider, String> {
    Provider::parse(provider).ok_or_else(|| format!("unknown provider {provider}"))
}

/// Choose what the thread's next send uses: a provider, a model and a
/// reasoning level. Stored at once, mid-turn or not: the running turn and
/// every run already written keep what they were sent with, and a message
/// that waits goes out under whatever is chosen when its turn comes.
/// Another provider is refused, with the reason, where its bridge cannot
/// take over a started chat (`providerHandoff`) or the current one has a
/// background task alive.
#[tauri::command]
pub async fn harness_select_model(
    state: State<'_, HarnessState>,
    command_id: String,
    thread_id: i64,
    provider: String,
    model: Option<String>,
    effort: Option<String>,
) -> Result<(), String> {
    let kind = Kind::SelectModel {
        provider: parse_provider(&provider)?,
        model,
        effort: validate_effort(effort)?,
    };
    let selected = state.command(Command {
        id: command_id,
        thread_id: Some(thread_id),
        kind,
    });
    selected.await.map(|_| ())
}

/// Ask a question again, differently: rewind the thread to that question
/// (rows and, where it can, the agent's context) and send the new text.
/// A provider that cannot rewind is not fatal — the rows still go, and
/// the `Rewound` event says the agent kept the original. `provider` is the
/// one the composer shows (null keeps the chat's): another one switches the
/// chat with this send, as [`harness_send`] does.
#[tauri::command]
pub async fn harness_edit_resend(
    state: State<'_, HarnessState>,
    command_id: String,
    thread_id: i64,
    item_id: i64,
    text: String,
    options: Option<SendOptions>,
    provider: Option<String>,
) -> Result<(), String> {
    let mut options = options.unwrap_or_default();
    options.reasoning_effort = validate_effort(options.reasoning_effort)?;
    let provider = provider.map(|p| parse_provider(&p)).transpose()?;
    let kind = Kind::EditResend {
        item_id,
        text,
        options,
        provider,
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

/// A thread as a reload reads it: rows, runs, what waits in its queue, the
/// answer text streamed so far and the revision that covers them, in one
/// read.
#[tauri::command]
pub async fn harness_load(
    state: State<'_, HarnessState>,
    thread_id: i64,
) -> Result<load::Snapshot, String> {
    // After startup's repair, so a run the last process left open is not
    // read as still running. A repair that failed is its commands' error,
    // not a reason to show an empty chat.
    let _ = state.ready().await;
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

/// Stop a run and hold what waits behind it. `run_id` is the run the
/// student saw running (null for a turn the CLI started itself, or when the
/// webview knows none): Stop for a run that has since finished touches
/// nothing. The turn ends when its CLI says so; waiting messages stay in
/// the queue, held, until [`harness_resume`] or the next message.
#[tauri::command]
pub async fn harness_interrupt(
    state: State<'_, HarnessState>,
    command_id: String,
    thread_id: i64,
    run_id: Option<i64>,
) -> Result<(), String> {
    let stopped = state.command(Command {
        id: command_id,
        thread_id: Some(thread_id),
        kind: Kind::Stop { run_id },
    });
    stopped.await.map(|_| ())
}

/// Lift the hold a stop, a failed turn or a restart put on the thread's
/// queue: its waiting messages go out again, in order.
#[tauri::command]
pub async fn harness_resume(
    state: State<'_, HarnessState>,
    command_id: String,
    thread_id: i64,
) -> Result<(), String> {
    let resumed = state.command(Command {
        id: command_id,
        thread_id: Some(thread_id),
        kind: Kind::Resume,
    });
    resumed.await.map(|_| ())
}

/// Force stop: kill what is answering `run_id`, for a CLI that ignored
/// Stop (`StopUnresponsive`). Claude and Antigravity lose that thread's
/// process; Codex and opencode lose their server, which ends every other
/// chat's turn on it too. A run that has since ended is left alone.
#[tauri::command]
pub async fn harness_force_stop(
    state: State<'_, HarnessState>,
    command_id: String,
    thread_id: i64,
    run_id: i64,
) -> Result<(), String> {
    let stopped = state.command(Command {
        id: command_id,
        thread_id: Some(thread_id),
        kind: Kind::ForceStop { run_id },
    });
    stopped.await.map(|_| ())
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
