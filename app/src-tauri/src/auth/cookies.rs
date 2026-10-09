//! The Canvas and Okta cookie jars, snapshotted from a live webview to disk and
//! replayed by the HTTP clients.

use tauri::{AppHandle, Manager};

/// The persisted Canvas cookie header. The session cookie is HttpOnly and
/// session-scoped, so it is snapshotted from the live webview at sign-in and
/// replayed by the HTTP client; that is what survives a restart.
fn cookie_file_path() -> std::path::PathBuf {
    crate::library::paths::cookie_path(&crate::library::paths::data_dir())
}

/// Reads the Canvas and Okta cookies from a live webview (HttpOnly included)
/// and writes each host's joined `name=value; ...` header to disk. Call it only
/// once Canvas has signed the webview in: before that the jar holds the
/// anonymous `canvas_session` Canvas hands every visitor.
pub fn save_session_cookie(app: &AppHandle) {
    let dir = crate::library::paths::data_dir();
    snapshot_cookies(
        app,
        crate::sources::canvas::CANVAS_BASE,
        &cookie_file_path(),
    );
    snapshot_cookies(
        app,
        &format!("https://{}", crate::auth::okta::SSO_HOST),
        &crate::library::paths::sso_cookie_path(&dir),
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
            .find(|(label, _)| label.starts_with(crate::shell::browser::LABEL_PREFIX))
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
            match crate::library::paths::write_private(path, &header) {
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
    std::fs::read_to_string(crate::library::paths::sso_cookie_path(
        &crate::library::paths::data_dir(),
    ))
    .unwrap_or_default()
}
