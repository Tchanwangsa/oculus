//! Tauri commands for the Canvas sign-in state.

use super::login_window::open_canvas_window;
use super::session::saved_session_probe;
use super::{auth_flag_path, AuthProbe, AuthState};
use std::sync::Arc;
use tauri::{AppHandle, Manager};

#[tauri::command]
pub fn get_auth_status(state: tauri::State<AuthState>) -> bool {
    let file_says_auth = auth_flag_path().exists();
    let mem_says_auth = *state.0.lock().unwrap();
    if file_says_auth && !mem_says_auth {
        *state.0.lock().unwrap() = true;
    }
    file_says_auth || mem_says_auth
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
    open_canvas_window(app, auth_flag);
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
    crate::library::paths::sign_out(&crate::library::paths::data_dir())
        .map_err(|e| e.to_string())?;
    Ok(())
}
