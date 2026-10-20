//! Tauri commands for device sharing, and the `companion-state` event to the
//! main webview after every change. Errors are `{code, message}`
//! (`error.rs`). Starting and stopping bind and tear down a server, so they
//! run off the command thread.

use super::assets::Assets;
use super::core::Config;
use super::error::{Code, CompanionError};
use super::grants::EndReason;
use super::wire::{CompanionState, Endpoints, ShareInfo};
use super::Companion;
use crate::library::files::canvases::authority;
use crate::shell::browser::MAIN;
use tauri::{AppHandle, Emitter, EventTarget, Manager, State};

pub const STATE_EVENT: &str = "companion-state";

impl Companion {
    /// The app's sharing over the global authority, emitting its state to
    /// the main webview. Sharing stays off until the first share.
    pub fn for_app(app: &AppHandle) -> Self {
        let companion = Self::new(authority::global(), Assets::for_app(app), Config::default());
        let events = app.clone();
        companion.on_state(move |state: &CompanionState| {
            if let Err(error) = events.emit_to(EventTarget::webview(MAIN), STATE_EVENT, state) {
                elog!("[companion] state event not delivered: {error}");
            }
        });
        companion
    }
}

/// End every share and stop the server, as on app exit.
pub fn shutdown(app: &AppHandle) {
    if let Some(companion) = app.try_state::<Companion>() {
        companion.stop_all(EndReason::Shutdown);
    }
}

async fn blocking<T: Send + 'static>(
    work: impl FnOnce() -> Result<T, CompanionError> + Send + 'static,
) -> Result<T, CompanionError> {
    crate::runtime::blocking::run(move || Ok(work()))
        .await
        .map_err(|error| CompanionError::new(Code::Server, error))?
}

#[tauri::command]
pub async fn companion_share_start(
    companion: State<'_, Companion>,
    file_id: i64,
) -> Result<ShareInfo, CompanionError> {
    let companion = companion.inner().clone();
    blocking(move || companion.share_start(file_id)).await
}

#[tauri::command]
pub async fn companion_share_stop(
    companion: State<'_, Companion>,
    file_id: i64,
) -> Result<(), CompanionError> {
    let companion = companion.inner().clone();
    blocking(move || {
        companion.share_stop(file_id);
        Ok(())
    })
    .await
}

#[tauri::command]
pub async fn companion_stop_all(companion: State<'_, Companion>) -> Result<(), CompanionError> {
    let companion = companion.inner().clone();
    blocking(move || {
        companion.stop_all(EndReason::Stopped);
        Ok(())
    })
    .await
}

#[tauri::command]
pub fn companion_shares(companion: State<'_, Companion>) -> Vec<ShareInfo> {
    companion.shares()
}

#[tauri::command]
pub fn companion_endpoints(companion: State<'_, Companion>) -> Endpoints {
    companion.endpoints()
}
