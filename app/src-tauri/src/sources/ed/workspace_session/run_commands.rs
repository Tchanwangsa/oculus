//! Run's commands (`run`). Output and the exit arrive as `run` and
//! `runExit` events.

use tauri::AppHandle;

use super::commands::session;
use super::events::RunStarted;
use super::terminal::PtySize;
use crate::runtime::blocking;

/// Run the challenge's `run_command` at the latest commit, stopping any run
/// before it. Its place in the save queue is taken before any waiting.
#[tauri::command]
pub async fn ed_workspace_run(
    app: AppHandle,
    challenge_id: i64,
    cols: u16,
    rows: u16,
) -> Result<RunStarted, String> {
    let session = session(&app, challenge_id)?;
    let place = session.place();
    blocking::run(move || session.run(place, PtySize { cols, rows })).await
}

#[tauri::command]
pub async fn ed_run_input(
    app: AppHandle,
    challenge_id: i64,
    run_id: String,
    data: String,
) -> Result<(), String> {
    session(&app, challenge_id)?.run_input(&run_id, &data)
}

#[tauri::command]
pub async fn ed_run_resize(
    app: AppHandle,
    challenge_id: i64,
    run_id: String,
    cols: u16,
    rows: u16,
) -> Result<(), String> {
    session(&app, challenge_id)?.run_resize(&run_id, PtySize { cols, rows })
}

/// Idempotent: a run that already ended is left as it is.
#[tauri::command]
pub async fn ed_run_stop(app: AppHandle, challenge_id: i64, run_id: String) -> Result<(), String> {
    session(&app, challenge_id)?.run_stop(&run_id);
    Ok(())
}
