use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter, Manager, WebviewUrl, WebviewWindowBuilder};

pub struct AuthState(pub Arc<Mutex<bool>>);

#[tauri::command]
fn get_auth_status(state: tauri::State<AuthState>) -> bool {
    *state.0.lock().unwrap()
}

#[tauri::command]
async fn launch_canvas_auth(
    app: AppHandle,
    state: tauri::State<'_, AuthState>,
) -> Result<(), String> {
    // Close existing auth window if open
    if let Some(existing) = app.get_webview_window("canvas-auth") {
        existing.close().map_err(|e| e.to_string())?;
    }

    let auth_flag = Arc::clone(&state.0);
    let auth_flag_win = Arc::clone(&state.0);
    let app_nav = app.clone();
    let app_win = app.clone();

    let win = WebviewWindowBuilder::new(
        &app,
        "canvas-auth",
        WebviewUrl::External(
            "https://canvas.lms.unimelb.edu.au/login/saml"
                .parse()
                .unwrap(),
        ),
    )
    .title("Sign in to Canvas — Oculus")
    .inner_size(900.0, 700.0)
    .center()
    .on_navigation(move |url| {
        let host_ok = url.host_str() == Some("canvas.lms.unimelb.edu.au");
        let path = url.path();

        // Positive allowlist — only these paths mean authenticated
        let authenticated = host_ok
            && (path == "/"
                || path.starts_with("/dashboard")
                || path.starts_with("/courses")
                || path.starts_with("/calendar")
                || path.starts_with("/inbox"));

        if authenticated {
            let already_done = {
                let mut flag = auth_flag.lock().unwrap();
                let prev = *flag;
                *flag = true;
                prev
            };
            if !already_done {
                if let Some(w) = app_nav.get_webview_window("canvas-auth") {
                    w.hide().ok();
                }
                app_nav.emit("canvas-auth-success", "ok").ok();
            }
        }
        true
    })
    .build()
    .map_err(|e| e.to_string())?;

    // CloseRequested fires when user clicks X — Destroyed may not propagate reliably
    win.on_window_event(move |event| {
        if let tauri::WindowEvent::CloseRequested { .. } = event {
            let authenticated = *auth_flag_win.lock().unwrap();
            if !authenticated {
                app_win.emit("canvas-auth-cancelled", "cancelled").ok();
            }
        }
    });

    Ok(())
}

#[tauri::command]
async fn disconnect_canvas(
    app: AppHandle,
    state: tauri::State<'_, AuthState>,
) -> Result<(), String> {
    // Reset auth flag
    *state.0.lock().unwrap() = false;

    if let Some(win) = app.get_webview_window("canvas-auth") {
        // Navigate to Canvas logout to clear server-side session
        // Then close the window — WebView2 cookies cleared on next open via fresh window
        win.eval("window.location.href = 'https://canvas.lms.unimelb.edu.au/logout'")
            .ok();
        // Short delay then close so logout request fires
        std::thread::sleep(std::time::Duration::from_millis(800));
        win.close().map_err(|e| e.to_string())?;
    }

    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .manage(AuthState(Arc::new(Mutex::new(false))))
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            get_auth_status,
            launch_canvas_auth,
            disconnect_canvas,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
