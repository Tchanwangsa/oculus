use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter, Manager, WebviewUrl, WebviewWindowBuilder};

pub struct AuthState(pub Arc<Mutex<bool>>);

pub const CANVAS_BASE: &str = "https://canvas.lms.unimelb.edu.au";

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

/// Attach the saved session cookie to an outgoing Canvas request.
pub fn apply_session(app: &AppHandle, req: ureq::Request) -> ureq::Request {
    let cookie = crate::files::proxy_cookie(app);
    if cookie.is_empty() {
        req
    } else {
        req.set("Cookie", &cookie)
    }
}

// ── Cookie rotation ──────────────────────────────────────────────────────────
//
// `canvas_session` carries no Expires/Max-Age — its lifetime is enforced
// server-side and extended by use, which is what the keep-alive ping is for.
// Canvas also re-issues the cookie itself now and then (not on every response,
// as measured). Keeping the login snapshot forever would mean discarding those
// rotations and eventually presenting a value the server has moved past, so
// every response we make is folded back into the store.

fn parse_cookie_header(header: &str) -> Vec<(String, String)> {
    header
        .split(';')
        .filter_map(|part| {
            let part = part.trim();
            if part.is_empty() {
                return None;
            }
            let (name, value) = part.split_once('=')?;
            Some((name.trim().to_string(), value.trim().to_string()))
        })
        .collect()
}

/// Merge `Set-Cookie` values into a cookie header. Returns the new header, or
/// `None` when nothing changed. Order is preserved so the header stays stable.
fn merged_cookie_header(current: &str, set_cookies: &[String]) -> Option<String> {
    if current.is_empty() {
        return None;
    }

    let mut pairs = parse_cookie_header(current);
    let mut changed = false;

    for raw in set_cookies {
        // "name=value; Path=/; HttpOnly" → we only care about the first pair.
        let Some(first) = raw.split(';').next() else {
            continue;
        };
        let Some((name, value)) = first.trim().split_once('=') else {
            continue;
        };
        let (name, value) = (name.trim(), value.trim());
        if name.is_empty() {
            continue;
        }

        match pairs.iter_mut().find(|(n, _)| n == name) {
            Some(slot) => {
                if slot.1 != value {
                    slot.1 = value.to_string();
                    changed = true;
                }
            }
            None => {
                pairs.push((name.to_string(), value.to_string()));
                changed = true;
            }
        }
    }

    if !changed {
        return None;
    }

    Some(
        pairs
            .iter()
            .map(|(n, v)| format!("{n}={v}"))
            .collect::<Vec<_>>()
            .join("; "),
    )
}

/// Fold `Set-Cookie` values into the stored header, writing only on a change —
/// a sync makes hundreds of requests and most carry no rotation.
fn merge_set_cookies(app: &AppHandle, set_cookies: &[String]) {
    let Some(header) = merged_cookie_header(&saved_cookie_header(app), set_cookies) else {
        return;
    };
    if let Err(e) = std::fs::write(cookie_file_path(app), &header) {
        eprintln!("[oculus] cookie refresh write failed: {e}");
    }
}

/// Fold rotated cookies from a response back into the store.
pub fn refresh_cookies(app: &AppHandle, resp: &ureq::Response) {
    let set: Vec<String> = resp
        .all("set-cookie")
        .into_iter()
        .map(str::to_string)
        .collect();
    if set.is_empty() {
        return;
    }
    merge_set_cookies(app, &set);
}

/// Outcome of pinging Canvas. `Unreachable` is deliberately distinct from
/// `Rejected` — a flat network is no reason to throw away a working session
/// and force a fresh SSO login.
pub enum AuthProbe {
    Valid(String),
    Rejected(String),
    Unreachable(String),
}

fn interpret_self(resp: Result<ureq::Response, ureq::Error>) -> AuthProbe {
    match resp {
        Ok(resp) => {
            let parsed: Result<serde_json::Value, _> = serde_json::from_reader(resp.into_reader());
            match parsed {
                Ok(v) => AuthProbe::Valid(
                    v["name"]
                        .as_str()
                        .or_else(|| v["short_name"].as_str())
                        .unwrap_or("Canvas user")
                        .to_string(),
                ),
                Err(e) => AuthProbe::Unreachable(format!("unreadable Canvas response: {e}")),
            }
        }
        Err(ureq::Error::Status(401, _)) => {
            AuthProbe::Rejected("Canvas rejected the session (401) — sign in again.".to_string())
        }
        Err(ureq::Error::Status(code, _)) => {
            AuthProbe::Rejected(format!("Canvas returned HTTP {code}"))
        }
        Err(e) => AuthProbe::Unreachable(format!("could not reach Canvas: {e}")),
    }
}

fn self_url() -> String {
    format!("{CANVAS_BASE}/api/v1/users/self")
}

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

/// Reads every cookie from the live login WebView (includes HttpOnly via the
/// native store) and writes the joined `name=value; ...` header to disk.
pub fn save_session_cookie(app: &AppHandle) {
    let Some(win) = app.get_webview_window("canvas-auth") else {
        return;
    };
    match win.cookies() {
        Ok(cookies) => {
            let header = cookies
                .iter()
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
/// rotated cookie is written back.
pub fn saved_session_probe(app: &AppHandle) -> AuthProbe {
    if !has_session(app) {
        return AuthProbe::Rejected("No saved Canvas session.".to_string());
    }

    let resp = apply_session(app, ureq::get(&self_url())).call();
    match &resp {
        Ok(r) | Err(ureq::Error::Status(_, r)) => refresh_cookies(app, r),
        Err(_) => {}
    }

    let probe = interpret_self(resp);
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
    let cookie = cookie_file_path(&app);
    if cookie.exists() {
        std::fs::remove_file(&cookie).map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sc(vals: &[&str]) -> Vec<String> {
        vals.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn replaces_rotated_value_in_place() {
        let out = merged_cookie_header(
            "a=1; canvas_session=OLD; z=9",
            &sc(&["canvas_session=NEW; path=/; secure; httponly"]),
        );
        // Position must be preserved, not appended to the end.
        assert_eq!(out.unwrap(), "a=1; canvas_session=NEW; z=9");
    }

    #[test]
    fn appends_cookies_not_seen_before() {
        let out = merged_cookie_header("a=1", &sc(&["b=2; path=/"]));
        assert_eq!(out.unwrap(), "a=1; b=2");
    }

    #[test]
    fn no_write_when_value_is_unchanged() {
        assert!(merged_cookie_header("a=1; b=2", &sc(&["b=2; path=/"])).is_none());
    }

    #[test]
    fn ignores_junk_and_empty_store() {
        assert!(merged_cookie_header("", &sc(&["a=1"])).is_none());
        assert!(merged_cookie_header("a=1", &sc(&["novalue; path=/"])).is_none());
    }

    #[test]
    fn keeps_base64_padding_in_values() {
        // Canvas session values are base64 and end in '='; splitting on the
        // first '=' only is what preserves them.
        let out = merged_cookie_header("s=old", &sc(&["s=abc==; path=/"])).unwrap();
        assert_eq!(out, "s=abc==");
    }
}
