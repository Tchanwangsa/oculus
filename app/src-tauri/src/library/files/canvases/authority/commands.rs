//! Tauri commands for desktop panes. Errors are `{code, message}`
//! (`error.rs`). Accepted changes and lifecycle moves reach the main webview as
//! `canvas-authority-change` and `canvas-authority-state` events.

use super::error::AuthorityError;
use super::events::Event;
use super::global;
use super::session::{Ack, Op, Resolution, Snapshot};
use crate::library::files::ImportedFile;
use crate::shell::browser::MAIN;
use serde_json::Value;
use std::sync::OnceLock;
use tauri::{AppHandle, Emitter, EventTarget};

pub const CHANGE_EVENT: &str = "canvas-authority-change";
pub const STATE_EVENT: &str = "canvas-authority-state";

/// Forward authority events to the main webview, once per process.
fn desktop_events(app: &AppHandle) {
    static REGISTERED: OnceLock<u64> = OnceLock::new();
    REGISTERED.get_or_init(|| {
        let app = app.clone();
        global().subscribers().subscribe(move |event| {
            let sent = match event {
                Event::Change(change) => {
                    app.emit_to(EventTarget::webview(MAIN), CHANGE_EVENT, change)
                }
                Event::State(state) => app.emit_to(EventTarget::webview(MAIN), STATE_EVENT, state),
            };
            if let Err(error) = sent {
                elog!("[canvas-authority] event not delivered: {error}");
            }
        })
    });
}

async fn blocking<T: Send + 'static>(
    work: impl FnOnce() -> Result<T, AuthorityError> + Send + 'static,
) -> Result<T, AuthorityError> {
    crate::runtime::blocking::run(move || Ok(work()))
        .await
        .map_err(AuthorityError::io)?
}

fn root() -> std::path::PathBuf {
    crate::library::paths::data_dir()
}

#[tauri::command]
pub async fn canvas_authority_open(
    app: AppHandle,
    file_id: i64,
    relative_path: String,
    text: String,
) -> Result<Snapshot, AuthorityError> {
    desktop_events(&app);
    blocking(move || global().open(file_id, &root(), &relative_path, &text)).await
}

#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub async fn canvas_authority_commit(
    file_id: i64,
    client_id: String,
    seq: u64,
    op_id: String,
    transaction: Value,
    viewport: Option<Value>,
    epoch: Option<String>,
) -> Result<Ack, AuthorityError> {
    let op = Op {
        client_id,
        seq,
        op_id,
        transaction,
        viewport,
        epoch,
    };
    blocking(move || global().commit(file_id, op)).await
}

#[tauri::command]
pub async fn canvas_authority_snapshot(file_id: i64) -> Result<Snapshot, AuthorityError> {
    blocking(move || global().snapshot(file_id)).await
}

#[tauri::command]
pub async fn canvas_authority_resolve_external(
    file_id: i64,
    choice: Resolution,
) -> Result<Snapshot, AuthorityError> {
    blocking(move || global().resolve_external(file_id, choice)).await
}

#[tauri::command]
pub async fn canvas_authority_rename(
    file_id: i64,
    title: String,
) -> Result<ImportedFile, AuthorityError> {
    blocking(move || global().rename(file_id, &title)).await
}

#[tauri::command]
pub async fn canvas_authority_delete(file_id: i64) -> Result<(), AuthorityError> {
    blocking(move || global().delete(file_id)).await
}

#[tauri::command]
pub async fn canvas_authority_close(file_id: i64) -> Result<(), AuthorityError> {
    blocking(move || {
        global().close(file_id);
        Ok(())
    })
    .await
}
