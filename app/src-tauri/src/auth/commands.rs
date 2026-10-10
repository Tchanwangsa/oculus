//! The Tauri commands the Settings page and the sign-in UI call.

use std::sync::Arc;

use tauri::{AppHandle, Manager};

use super::{login_window, saved_session_probe, session, AuthProbe, AuthState};
use crate::providers::credentials::Credentialed;

/// Whether the app counts as signed in. The answer comes from oculus-keyd, so
/// it runs off the main thread.
#[tauri::command]
pub async fn get_auth_status(state: tauri::State<'_, AuthState>) -> Result<bool, String> {
    let memory = Arc::clone(&state.0);
    tauri::async_runtime::spawn_blocking(move || {
        session::authenticated(
            &Credentialed::at(&crate::library::paths::data_dir()),
            &memory,
        )
    })
    .await
    .map_err(|e| e.to_string())
}

/// Live session check for the UI; `get_auth_status` only says a sign-in once
/// happened. `unreachable` means inconclusive — keep the current state.
#[tauri::command]
pub async fn check_canvas_session() -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(|| match saved_session_probe() {
        AuthProbe::Valid(_) => "valid".to_string(),
        AuthProbe::Rejected(_) => "expired".to_string(),
        AuthProbe::Unreachable(_) => "unreachable".to_string(),
    })
    .await
    .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn launch_canvas_auth(
    app: AppHandle,
    state: tauri::State<'_, AuthState>,
) -> Result<(), String> {
    let auth_flag = Arc::clone(&state.0);
    login_window::open_canvas_window(app, auth_flag);
    Ok(())
}

#[tauri::command]
pub async fn disconnect_canvas(
    app: AppHandle,
    state: tauri::State<'_, AuthState>,
) -> Result<(), String> {
    *state.0.lock().unwrap() = false;

    if let Some(win) = app.get_webview_window("canvas-auth") {
        win.close().map_err(|e| e.to_string())?;
    }

    // The jar first, or a Canvas tab saves the session straight back.
    let (cleared, done) = tokio::sync::oneshot::channel();
    crate::shell::browser::clear_sessions(&app, move || {
        cleared.send(()).ok();
    });
    done.await.ok();
    tauri::async_runtime::spawn_blocking(|| {
        session::sign_out(&Credentialed::at(&crate::library::paths::data_dir()))
    })
    .await
    .map_err(|e| e.to_string())??;
    Ok(())
}
