use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter, Manager, WebviewUrl, WebviewWindowBuilder};

pub struct AuthState(pub Arc<Mutex<bool>>);

pub fn canvas_session_dir(app: &AppHandle) -> std::path::PathBuf {
    app.path()
        .app_data_dir()
        .expect("no app data dir")
        .join("canvas-session")
}

pub fn auth_flag_path(app: &AppHandle) -> std::path::PathBuf {
    canvas_session_dir(app).join("authenticated")
}

pub fn is_authenticated_url(url: &url::Url) -> bool {
    url.host_str() == Some("canvas.lms.unimelb.edu.au")
        && {
            let p = url.path();
            p == "/"
                || p.starts_with("/dashboard")
                || p.starts_with("/courses")
                || p.starts_with("/calendar")
                || p.starts_with("/inbox")
        }
}

pub fn open_canvas_window(app: AppHandle, auth_flag: Arc<Mutex<bool>>, silent: bool) {
    if let Some(existing) = app.get_webview_window("canvas-auth") {
        existing.close().ok();
        std::thread::sleep(std::time::Duration::from_millis(120));
    }

    let url = "https://canvas.lms.unimelb.edu.au/login/saml";

    let session_dir = canvas_session_dir(&app);
    let flag_path = auth_flag_path(&app);
    let app_nav = app.clone();
    let app_win = app.clone();
    let auth_flag_nav = Arc::clone(&auth_flag);
    let auth_flag_win = Arc::clone(&auth_flag);

    let resolved = Arc::new(AtomicBool::new(false));
    let resolved_nav = Arc::clone(&resolved);

    let result = WebviewWindowBuilder::new(
        &app,
        "canvas-auth",
        WebviewUrl::External(url.parse().unwrap()),
    )
    .title("Sign in to Canvas — Oculus")
    .inner_size(900.0, 700.0)
    .center()
    .visible(!silent)
    .data_directory(session_dir)
    .on_navigation(move |url| {
        if is_authenticated_url(&url) {
            let was_resolved = resolved_nav.swap(true, Ordering::SeqCst);
            if !was_resolved {
                *auth_flag_nav.lock().unwrap() = true;
                std::fs::create_dir_all(flag_path.parent().unwrap()).ok();
                std::fs::write(&flag_path, b"1").ok();

                if let Some(w) = app_nav.get_webview_window("canvas-auth") {
                    w.hide().ok();
                }
                app_nav.emit("canvas-auth-success", "ok").ok();
                eprintln!(
                    "[oculus] auth success ({})",
                    if silent { "silent" } else { "interactive" }
                );
            }
        }
        true
    })
    .build();

    let win = match result {
        Ok(w) => w,
        Err(e) => {
            eprintln!("[oculus] failed to open canvas window: {e}");
            return;
        }
    };

    if silent {
        // Silent restore: give the SSO redirect chain time to finish using the
        // persisted IdP cookies in `data_directory`. If it can't auto-complete
        // (genuine expiry, or the IdP wants an MFA / "stay signed in?" tap), do
        // NOT wipe anything — just reveal the window so the user finishes
        // interactively (usually one tap, not a full re-login). Cookies are
        // only ever cleared by explicit Disconnect. Wiping here was the bug
        // that made auth "never persist": one timeout poisoned the session.
        let app_to = app.clone();
        std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_secs(15));
            if !resolved.load(Ordering::SeqCst) {
                eprintln!("[oculus] silent restore didn't auto-complete — revealing window for interactive SSO (cookies kept)");
                if let Some(w) = app_to.get_webview_window("canvas-auth") {
                    w.show().ok();
                    w.set_focus().ok();
                }
            }
        });
    } else {
        win.on_window_event(move |event| {
            if let tauri::WindowEvent::CloseRequested { .. } = event {
                if !*auth_flag_win.lock().unwrap() {
                    app_win.emit("canvas-auth-cancelled", "cancelled").ok();
                }
            }
        });
    }
}

#[tauri::command]
pub fn get_auth_status(app: AppHandle, state: tauri::State<AuthState>) -> bool {
    let flag = auth_flag_path(&app);
    eprintln!("[oculus] get_auth_status — flag path: {}", flag.display());
    let file_says_auth = flag.exists();
    let mem_says_auth = *state.0.lock().unwrap();
    eprintln!("[oculus] get_auth_status — file={file_says_auth} mem={mem_says_auth}");

    if file_says_auth && !mem_says_auth {
        *state.0.lock().unwrap() = true;
    }

    file_says_auth || mem_says_auth
}

#[tauri::command]
pub fn open_canvas_devtools(app: AppHandle) {
    if let Some(win) = app.get_webview_window("canvas-auth") {
        win.show().ok();
        win.open_devtools();
    }
}

#[tauri::command]
pub async fn launch_canvas_auth(
    app: AppHandle,
    state: tauri::State<'_, AuthState>,
) -> Result<(), String> {
    let auth_flag = Arc::clone(&state.0);
    open_canvas_window(app, auth_flag, false);
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

    let session_dir = canvas_session_dir(&app);
    if session_dir.exists() {
        std::fs::remove_dir_all(&session_dir).map_err(|e| e.to_string())?;
    }

    Ok(())
}
