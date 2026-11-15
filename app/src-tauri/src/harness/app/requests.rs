//! What the student does about a request, and how a chat asks: Tauri
//! commands that build a [`Command`] for the consumer, as `turns.rs` does.

use tauri::State;

use crate::harness::core::{Command, Kind};
use crate::harness::requests::PermissionMode;

use super::HarnessState;

/// Answer a request with one of its options (`HarnessRequest.options`).
/// Refused with the reason to show when the request is not this chat's,
/// was already answered, has ended, or its agent has gone. Returns once
/// the answer is saved; its delivery follows on `request_updated`.
#[tauri::command]
pub async fn harness_respond(
    state: State<'_, HarnessState>,
    command_id: String,
    thread_id: i64,
    request_id: i64,
    option_id: String,
    feedback: Option<String>,
) -> Result<(), String> {
    let answered = state.command(Command {
        id: command_id,
        thread_id: Some(thread_id),
        kind: Kind::Respond {
            request_id,
            option_id,
            feedback,
        },
    });
    answered.await.map(|_| ())
}

/// How the chat asks from its next run on: `ask`, `quiet` ("Don't ask")
/// or `auto`. Refused while a request waits.
#[tauri::command]
pub async fn harness_set_permission_mode(
    state: State<'_, HarnessState>,
    command_id: String,
    thread_id: i64,
    mode: PermissionMode,
) -> Result<(), String> {
    let set = state.command(Command {
        id: command_id,
        thread_id: Some(thread_id),
        kind: Kind::SetPermissionMode { mode },
    });
    set.await.map(|_| ())
}
