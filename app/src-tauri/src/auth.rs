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

fn cookies_path(app: &AppHandle) -> std::path::PathBuf {
    app.path()
        .app_data_dir()
        .expect("no app data dir")
        .join("canvas-cookies.json")
}

pub fn is_authenticated_url(url: &url::Url) -> bool {
    url.host_str() == Some("canvas.lms.unimelb.edu.au")
        && {
            let p = url.path();
            let q = url.query().unwrap_or("");
            // Exclude transient /?login_success=1 hop — session cookie not
            // set yet and Canvas hasn't JS-redirected to the real dashboard.
            (p == "/" && !q.contains("login_success"))
                || p.starts_with("/dashboard")
                || p.starts_with("/courses")
                || p.starts_with("/calendar")
                || p.starts_with("/inbox")
        }
}

// ── Cookie persistence ────────────────────────────────────────────────────────
// Session cookies die when WebView2 exits — they're `is_persistent=0` in
// Chromium.  We snapshot them here while the window is alive so we can do
// a fast server-side liveness check on the next launch without opening a
// hidden WebView that gets stuck on the SSO login page.

fn save_cookies(app: &AppHandle) {
    if let Some(w) = app.get_webview_window("canvas-auth") {
        if let Ok(cookies) = w.cookies() {
            let list: Vec<serde_json::Value> = cookies
                .iter()
                .map(|c| {
                    serde_json::json!({
                        "name": c.name(),
                        "value": c.value(),
                    })
                })
                .collect();
            if let Ok(json) = serde_json::to_string(&list) {
                let path = cookies_path(app);
                std::fs::write(&path, json).ok();
                eprintln!("[oculus] saved {} cookies to {}", list.len(), path.display());
            }
        }
    }
}

fn load_cookie_header(app: &AppHandle) -> String {
    let path = cookies_path(app);
    match std::fs::read_to_string(&path) {
        Ok(data) => match serde_json::from_str::<Vec<serde_json::Value>>(&data) {
            Ok(list) => list
                .iter()
                .filter_map(|c| {
                    let name = c["name"].as_str()?;
                    let value = c["value"].as_str()?;
                    Some(format!("{}={}", name, value))
                })
                .collect::<Vec<_>>()
                .join("; "),
            Err(_) => String::new(),
        },
        Err(_) => String::new(),
    }
}

/// Pings the Canvas API with cached cookies.  Returns true iff the server still
/// accepts them — no WebView window needed.
pub fn check_cached_session(app: &AppHandle) -> bool {
    let cookie = load_cookie_header(app);
    if cookie.is_empty() {
        return false;
    }

    match ureq::get("https://canvas.lms.unimelb.edu.au/api/v1/users/self")
        .set("Cookie", &cookie)
        .call()
    {
        Ok(resp) => {
            let ok = resp.status() == 200;
            eprintln!("[oculus] cookie check: HTTP {} → {}", resp.status(), if ok { "valid" } else { "expired" });
            ok
        }
        Err(e) => {
            eprintln!("[oculus] cookie check failed: {e}");
            false
        }
    }
}

// ── Window / auth flow ────────────────────────────────────────────────────────

pub fn open_canvas_window(app: AppHandle, auth_flag: Arc<Mutex<bool>>) {
    if let Some(existing) = app.get_webview_window("canvas-auth") {
        existing.close().ok();
        std::thread::sleep(std::time::Duration::from_millis(300));
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
    .visible(true)
    .data_directory(session_dir)
    .on_navigation(move |url| {
        eprintln!("[oculus] nav: {}", url);

        if is_authenticated_url(&url) {
            let was_resolved = resolved_nav.swap(true, Ordering::SeqCst);
            if !was_resolved {
                *auth_flag_nav.lock().unwrap() = true;
                std::fs::create_dir_all(flag_path.parent().unwrap()).ok();
                std::fs::write(&flag_path, b"1").ok();

                app_nav.emit("canvas-auth-success", "ok").ok();
                eprintln!("[oculus] auth success");

                let app_delayed = app_nav.clone();
                std::thread::spawn(move || {
                    std::thread::sleep(std::time::Duration::from_secs(2));
                    save_cookies(&app_delayed);
                    if let Some(w) = app_delayed.get_webview_window("canvas-auth") {
                        w.hide().ok();
                    }
                });
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

    win.on_window_event(move |event| {
        if let tauri::WindowEvent::CloseRequested { .. } = event {
            if !*auth_flag_win.lock().unwrap() {
                app_win.emit("canvas-auth-cancelled", "cancelled").ok();
            }
        }
    });
}

// ── Tauri commands ────────────────────────────────────────────────────────────

#[tauri::command]
pub fn get_auth_status(app: AppHandle, state: tauri::State<AuthState>) -> bool {
    let flag = auth_flag_path(&app);
    let file_says_auth = flag.exists();
    let mem_says_auth = *state.0.lock().unwrap();

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

    let session_dir = canvas_session_dir(&app);
    if session_dir.exists() {
        std::fs::remove_dir_all(&session_dir).map_err(|e| e.to_string())?;
    }

    let cp = cookies_path(&app);
    if cp.exists() {
        std::fs::remove_file(&cp).map_err(|e| e.to_string())?;
    }

    Ok(())
}
