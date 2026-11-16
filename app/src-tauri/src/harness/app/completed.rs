//! Complete and Reopen use the same receipt and commit path as sends.

use super::HarnessState;
use crate::harness::core::{Command, Kind};
use tauri::{AppHandle, Emitter, Manager, State};

#[tauri::command]
pub async fn harness_complete_thread(
    state: State<'_, HarnessState>,
    command_id: String,
    thread_id: i64,
) -> Result<(), String> {
    state
        .command(Command {
            id: command_id,
            thread_id: Some(thread_id),
            kind: Kind::CompleteThread,
        })
        .await
        .map(|_| ())
}

#[tauri::command]
pub async fn harness_reopen_thread(
    state: State<'_, HarnessState>,
    command_id: String,
    thread_id: i64,
) -> Result<(), String> {
    state
        .command(Command {
            id: command_id,
            thread_id: Some(thread_id),
            kind: Kind::ReopenThread,
        })
        .await
        .map(|_| ())
}

pub(super) fn start_sweep(app: &AppHandle) {
    let app = app.clone();
    let startup = app.state::<HarnessState>().startup.clone();
    tauri::async_runtime::spawn(async move {
        loop {
            if crate::db::store::db_path().exists() {
                let result = async {
                    startup.ready(crate::db::store::open_pool).await?;
                    let pool = crate::db::store::open_pool().await?;
                    crate::harness::store::completed::sweep(&pool).await
                }
                .await;
                match result {
                    Ok(ids) if !ids.is_empty() => {
                        let _ = app.emit("harness-threads-changed", ids);
                    }
                    Ok(_) => {}
                    Err(e) => elog!("[oculus] harness completion sweep: {e}"),
                }
            }
            tokio::time::sleep(std::time::Duration::from_secs(60 * 60)).await;
        }
    });
}
