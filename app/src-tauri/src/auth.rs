use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter, Manager, WebviewUrl, WebviewWindowBuilder};

pub struct AuthState(pub Arc<Mutex<bool>>);

fn canvas_session_dir() -> std::path::PathBuf {
    crate::paths::data_dir().join("canvas-session")
}

pub fn auth_flag_path() -> std::path::PathBuf {
    crate::paths::auth_flag_path(&crate::paths::data_dir())
}

/// The persisted Canvas cookie header. The session cookie is HttpOnly and
/// session-scoped, so it is snapshotted from the live webview at sign-in and
/// replayed by the HTTP client; that is what survives a restart.
fn cookie_file_path() -> std::path::PathBuf {
    crate::paths::cookie_path(&crate::paths::data_dir())
}

pub use crate::canvas::SessionProbe as AuthProbe;

pub fn is_authenticated_url(url: &url::Url) -> bool {
    url.host_str() == Some("canvas.lms.unimelb.edu.au") && {
        let p = url.path();
        // `/?login_success=1` is the success signal: the JS redirect to the
        // dashboard after it fires no nav event.
        p == "/"
            || p.starts_with("/dashboard")
            || p.starts_with("/courses")
            || p.starts_with("/calendar")
            || p.starts_with("/inbox")
    }
}

// ── Cookie snapshot / replay ────────────────────────────────────────────────

/// Reads the Canvas and Okta cookies from a live webview (HttpOnly included)
/// and writes each host's joined `name=value; ...` header to disk. Call it only
/// once Canvas has signed the webview in: before that the jar holds the
/// anonymous `canvas_session` Canvas hands every visitor.
pub fn save_session_cookie(app: &AppHandle) {
    let dir = crate::paths::data_dir();
    snapshot_cookies(app, crate::canvas::CANVAS_BASE, &cookie_file_path());
    snapshot_cookies(
        app,
        &format!("https://{}", crate::okta::SSO_HOST),
        &crate::paths::sso_cookie_path(&dir),
    );
}

/// Scoped with `cookies_for_url`: every webview shares one jar
/// (`data_directory` is a no-op on WKWebView).
fn snapshot_cookies(app: &AppHandle, base: &str, path: &std::path::Path) {
    let Ok(base) = base.parse::<url::Url>() else {
        return;
    };
    // Same jar either way; fall back to an in-app browser tab when the login
    // window is gone.
    let cookies = match app.get_webview_window("canvas-auth") {
        Some(win) => win.cookies_for_url(base.clone()),
        None => match app
            .webviews()
            .into_iter()
            .find(|(label, _)| label.starts_with(crate::browser::LABEL_PREFIX))
        {
            Some((_, webview)) => webview.cookies_for_url(base.clone()),
            None => return,
        },
    };
    let host = base.host_str().unwrap_or_default();
    match cookies {
        Ok(cookies) => {
            // One entry per name: parent-domain cookies and the host copies
            // `browser::seed_sessions` writes would otherwise both be
            // replayed, growing the snapshot every load.
            let mut seen = std::collections::HashSet::new();
            let header = cookies
                .iter()
                .filter(|c| seen.insert(c.name().to_string()))
                .map(|c| format!("{}={}", c.name(), c.value()))
                .collect::<Vec<_>>()
                .join("; ");
            if header.is_empty() {
                eprintln!("[oculus] save_session_cookie: no {host} cookies to save yet");
                return;
            }
            match crate::paths::write_private(path, &header) {
                Ok(_) => eprintln!("[oculus] saved {host} cookies ({} bytes)", header.len()),
                Err(e) => eprintln!("[oculus] save_session_cookie {host} write failed: {e}"),
            }
        }
        Err(e) => eprintln!("[oculus] save_session_cookie {host}: cookies() failed: {e}"),
    }
}

/// The persisted cookie header, or empty if none saved.
pub fn saved_cookie_header() -> String {
    std::fs::read_to_string(cookie_file_path()).unwrap_or_default()
}

/// The persisted Okta header, or empty: written by a headless sign-in or a
/// signed-in browser page, read only by the in-app browser.
pub fn saved_sso_cookie_header() -> String {
    std::fs::read_to_string(crate::paths::sso_cookie_path(&crate::paths::data_dir()))
        .unwrap_or_default()
}

/// Pings the Canvas API with the saved session cookie. Doubles as the
/// keep-alive: the client writes back the rotated cookie.
pub fn saved_session_probe() -> AuthProbe {
    let probe = crate::canvas::Canvas::open(&crate::paths::data_dir()).probe();

    match &probe {
        AuthProbe::Valid(name) => eprintln!("[oculus] session check: valid ({name})"),
        AuthProbe::Rejected(why) => eprintln!("[oculus] session check: rejected — {why}"),
        AuthProbe::Unreachable(why) => eprintln!("[oculus] session check: inconclusive — {why}"),
    }
    probe
}

// ── Session established ─────────────────────────────────────────────────────

/// How a sign-in happened, for the one place every sign-in in the app ends.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Via {
    Window,
    Browser,
    Headless,
}

/// Every sign-in in the app ends here once its cookie is on disk: the auth
/// flag (which also lifts a sign-out), the in-memory state and the UI event.
/// A person's sign-in also clears the attempt guard's wait and pause; a
/// headless one has already settled the guard.
pub fn session_established(app: &AppHandle, dir: &std::path::Path, via: Via) {
    crate::paths::mark_authenticated(dir);
    if via != Via::Headless {
        crate::okta::resume_automatic_sign_in(dir);
    }
    if let Some(state) = app.try_state::<AuthState>() {
        *state.0.lock().unwrap() = true;
    }
    app.emit("canvas-auth-success", "ok").ok();
}

static CONFIRMING: AtomicBool = AtomicBool::new(false);

/// A browser tab reached a signed-in Canvas page while the app is not
/// connected: someone signed in by hand there. Its snapshot is already saved;
/// once Canvas accepts it, connect the app as the login window would.
pub fn confirm_browser_sign_in(app: &AppHandle) {
    if auth_flag_path().exists() || CONFIRMING.swap(true, Ordering::SeqCst) {
        return;
    }
    let app = app.clone();
    std::thread::spawn(move || {
        let dir = crate::paths::data_dir();
        match crate::canvas::Canvas::open(&dir).whoami() {
            Ok(name) => {
                eprintln!("[oculus] signed in from a browser tab as {name}");
                session_established(&app, &dir, Via::Browser);
            }
            Err(e) => eprintln!(
                "[oculus] browser tab looked signed in, but Canvas refused the session: {e}"
            ),
        }
        CONFIRMING.store(false, Ordering::SeqCst);
    });
}

// ── Login window (interactive only) ──────────────────────────────────────────

/// Opens the visible Canvas SAML login window. On success it writes the auth
/// flag, snapshots the session cookie, then hides itself.
pub fn open_canvas_window(app: AppHandle, auth_flag: Arc<Mutex<bool>>) {
    if let Some(existing) = app.get_webview_window("canvas-auth") {
        existing.close().ok();
        std::thread::sleep(std::time::Duration::from_millis(200));
    }

    let url = "https://canvas.lms.unimelb.edu.au/login/saml";

    let session_dir = canvas_session_dir();
    let dir = crate::paths::data_dir();
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
                // Now, so closing the window meanwhile is not a cancel.
                *auth_flag_nav.lock().unwrap() = true;

                // Give Canvas a moment to set the session cookie first.
                let app_delayed = app_nav.clone();
                let dir = dir.clone();
                std::thread::spawn(move || {
                    std::thread::sleep(std::time::Duration::from_secs(2));
                    save_session_cookie(&app_delayed);
                    session_established(&app_delayed, &dir, Via::Window);
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
    crate::browser::clear_sessions(&app, move || {
        cleared.send(()).ok();
    });
    done.await.ok();
    crate::paths::sign_out(&crate::paths::data_dir()).map_err(|e| e.to_string())?;
    Ok(())
}
