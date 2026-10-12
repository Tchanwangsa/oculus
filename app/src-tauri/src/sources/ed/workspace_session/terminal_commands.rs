//! The terminal drawer's commands (`terminal`). Output and state arrive as
//! `term` and `termState` events.

use tauri::AppHandle;

use super::commands::session;
use super::events::TerminalOpened;
use super::terminal::PtySize;

/// A terminal: a persisting one re-attached (`resumed`), else a new shell.
#[tauri::command]
pub async fn ed_terminal_open(
    app: AppHandle,
    challenge_id: i64,
    cols: u16,
    rows: u16,
) -> Result<TerminalOpened, String> {
    session(&app, challenge_id)?.terminal_open(PtySize { cols, rows })
}

/// `data` is the UTF-8 text xterm produced; it goes to Ed as base64.
#[tauri::command]
pub async fn ed_terminal_input(
    app: AppHandle,
    challenge_id: i64,
    id: String,
    data: String,
) -> Result<(), String> {
    session(&app, challenge_id)?.terminal_input(&id, &data)
}

#[tauri::command]
pub async fn ed_terminal_resize(
    app: AppHandle,
    challenge_id: i64,
    id: String,
    cols: u16,
    rows: u16,
) -> Result<(), String> {
    session(&app, challenge_id)?.terminal_resize(&id, PtySize { cols, rows })
}

#[tauri::command]
pub async fn ed_terminal_kill(app: AppHandle, challenge_id: i64, id: String) -> Result<(), String> {
    session(&app, challenge_id)?.terminal_kill(&id)
}
