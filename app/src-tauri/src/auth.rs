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

/// Persisted Canvas session cookie header. WebView2 keeps the real session
/// cookie in RAM only (it's HttpOnly + session-scoped, so Chromium never
/// writes it to disk and it can't be injected back into a WebView). We snapshot
/// it here while the login window is alive, then replay it ourselves via ureq
/// for every Canvas request — that's what survives a restart.
fn cookie_file_path(app: &AppHandle) -> std::path::PathBuf {
    app.path()
        .app_data_dir()
        .expect("no app data dir")
        .join("canvas-session.cookie")
}

/// True if we hold a session cookie to authenticate with.
pub fn has_session(app: &AppHandle) -> bool {
    !crate::files::proxy_cookie(app).is_empty()
}

/// Outcome of pinging Canvas. One definition, in the client that does the
/// pinging, so the app and the `oculus` CLI cannot drift on what "expired"
/// means.
pub use crate::canvas::SessionProbe as AuthProbe;

pub fn is_authenticated_url(url: &url::Url) -> bool {
    url.host_str() == Some("canvas.lms.unimelb.edu.au") && {
        let p = url.path();
        // `/?login_success=1` IS the success signal — Canvas then JS-redirects
        // to the dashboard, which fires no nav event, so we must catch it here.
        // (DeepSeek excluded login_success and broke detection.) We delay the
        // cookie snapshot 2s, by which point the session cookie is set.
        p == "/"
            || p.starts_with("/dashboard")
            || p.starts_with("/courses")
            || p.starts_with("/calendar")
            || p.starts_with("/inbox")
    }
}

// ── Cookie snapshot / replay ────────────────────────────────────────────────

/// Reads the Canvas cookies from a live WebView (includes HttpOnly via the
/// native store) and writes the joined `name=value; ...` header to disk.
///
/// **Canvas cookies only.** Every webview in the app shares one jar —
/// `data_directory` is a no-op on WKWebView — so `cookies()` would return
/// whatever the in-app browser has picked up anywhere on the web, and this
/// header is replayed verbatim to Canvas on every scrape. `cookies_for_url`
/// scopes it to what Canvas would actually be sent.
pub fn save_session_cookie(app: &AppHandle) {
    let Ok(canvas) = crate::canvas::CANVAS_BASE.parse::<url::Url>() else {
        return;
    };
    // Either live webview will do — same jar. A Canvas page open in the
    // browser has the *fresher* session, so fall back to it when the login
    // window is gone.
    let cookies = match app.get_webview_window("canvas-auth") {
        Some(win) => win.cookies_for_url(canvas),
        None => match app
            .webviews()
            .into_iter()
            .find(|(label, _)| label.starts_with(crate::browser::LABEL_PREFIX))
        {
            Some((_, webview)) => webview.cookies_for_url(canvas),
            None => return,
        },
    };
    match cookies {
        Ok(cookies) => {
            // One entry per name. WebKit hands back both the cookies UniMelb
            // sets on the parent domain and the copies the in-app browser
            // seeds on the Canvas host (`browser::seed_canvas_session`);
            // replaying both would grow the snapshot on every page load.
            let mut seen = std::collections::HashSet::new();
            let header = cookies
                .iter()
                .filter(|c| seen.insert(c.name().to_string()))
                .map(|c| format!("{}={}", c.name(), c.value()))
                .collect::<Vec<_>>()
                .join("; ");
            if header.is_empty() {
                eprintln!("[oculus] save_session_cookie: no cookies to save yet");
                return;
            }
            let path = cookie_file_path(app);
            match std::fs::write(&path, &header) {
                Ok(_) => eprintln!("[oculus] saved session cookie ({} bytes)", header.len()),
                Err(e) => eprintln!("[oculus] save_session_cookie write failed: {e}"),
            }
        }
        Err(e) => eprintln!("[oculus] save_session_cookie: cookies() failed: {e}"),
    }
}

/// The persisted cookie header, or empty if none saved.
pub fn saved_cookie_header(app: &AppHandle) -> String {
    std::fs::read_to_string(cookie_file_path(app)).unwrap_or_default()
}

/// Pings the Canvas API with the saved session cookie — no WebView needed.
/// Doubles as the keep-alive: the request rolls the session forward and the
/// rotated cookie is written back by the client.
pub fn saved_session_probe(app: &AppHandle) -> AuthProbe {
    let Ok(dir) = app.path().app_data_dir() else {
        return AuthProbe::Unreachable("no app data directory".to_string());
    };
    let probe = crate::canvas::Canvas::open(&dir).probe();

    match &probe {
        AuthProbe::Valid(name) => eprintln!("[oculus] session check: valid ({name})"),
        AuthProbe::Rejected(why) => eprintln!("[oculus] session check: rejected — {why}"),
        AuthProbe::Unreachable(why) => eprintln!("[oculus] session check: inconclusive — {why}"),
    }
    probe
}

// ── Login window (interactive only) ──────────────────────────────────────────

/// Opens the visible Canvas SAML login window. On success it writes the auth
/// flag, snapshots the session cookie, then hides itself. This is the ONLY
/// path that needs a WebView — all data fetching goes through the cookie proxy.
pub fn open_canvas_window(app: AppHandle, auth_flag: Arc<Mutex<bool>>) {
    if let Some(existing) = app.get_webview_window("canvas-auth") {
        existing.close().ok();
        std::thread::sleep(std::time::Duration::from_millis(200));
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
        eprintln!("[oculus] nav: {url}");
        if is_authenticated_url(&url) {
            let was_resolved = resolved_nav.swap(true, Ordering::SeqCst);
            if !was_resolved {
                *auth_flag_nav.lock().unwrap() = true;
                std::fs::create_dir_all(flag_path.parent().unwrap()).ok();
                std::fs::write(&flag_path, b"1").ok();

                // Give Canvas a moment to finish setting the session cookie
                // before we snapshot it, then hide the window.
                let app_delayed = app_nav.clone();
                std::thread::spawn(move || {
                    std::thread::sleep(std::time::Duration::from_secs(2));
                    save_session_cookie(&app_delayed);
                    app_delayed.emit("canvas-auth-success", "ok").ok();
                    if let Some(w) = app_delayed.get_webview_window("canvas-auth") {
                        w.hide().ok();
                    }
                    eprintln!("[oculus] auth success");
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
    let file_says_auth = auth_flag_path(&app).exists();
    let mem_says_auth = *state.0.lock().unwrap();
    if file_says_auth && !mem_says_auth {
        *state.0.lock().unwrap() = true;
    }
    file_says_auth || mem_says_auth
}

/// Live session check for the UI. Unlike `get_auth_status` — which only says
/// a sign-in once happened — this actually pings Canvas, so the settings page
/// can show "expired" instead of a stale "Active". `unreachable` means the
/// network answered nothing conclusive; the UI should keep its current state.
#[tauri::command]
pub async fn check_canvas_session(app: AppHandle) -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(move || match saved_session_probe(&app) {
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

    let session_dir = canvas_session_dir(&app);
    if session_dir.exists() {
        std::fs::remove_dir_all(&session_dir).map_err(|e| e.to_string())?;
    }
    let cookie = cookie_file_path(&app);
    if cookie.exists() {
        std::fs::remove_file(&cookie).map_err(|e| e.to_string())?;
    }
    Ok(())
}
