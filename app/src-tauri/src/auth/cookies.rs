//! Reading the Canvas and Okta cookies out of a live webview and handing them
//! to oculus-keyd, which holds the session from then on. The Canvas session
//! cookie is HttpOnly and session-scoped, so the webview's jar is the only
//! place a sign-in made there can be read from.

use keyd_core::client::SessionKind as Kind;
use tauri::{AppHandle, Manager};

use super::{session, tab};
use crate::providers::credentials::Credentialed;

/// The `name=value; ...` header for a jar's cookies, one entry per name:
/// parent-domain cookies and the host copies `browser::seed_sessions` writes
/// would otherwise both be kept, growing the snapshot every load.
fn cookie_header<'a>(cookies: impl IntoIterator<Item = (&'a str, &'a str)>) -> String {
    let mut seen = std::collections::HashSet::new();
    cookies
        .into_iter()
        .filter(|(name, _)| seen.insert(*name))
        .map(|(name, value)| format!("{name}={value}"))
        .collect::<Vec<_>>()
        .join("; ")
}

/// The Canvas and Okta cookies from a live webview (HttpOnly included), as
/// the sessions to store. An empty header means that host had none yet. Only
/// reads the jar; no socket call.
fn snapshot(app: &AppHandle) -> Vec<(Kind, String)> {
    vec![
        (
            Kind::Canvas,
            webview_header(app, crate::sources::canvas::CANVAS_BASE),
        ),
        (
            Kind::Sso,
            webview_header(app, &format!("https://{}", crate::auth::okta::SSO_HOST)),
        ),
    ]
}

/// Scoped with `cookies_for_url`: every webview shares one jar
/// (`data_directory` is a no-op on WKWebView).
fn webview_header(app: &AppHandle, base: &str) -> String {
    let Ok(base) = base.parse::<url::Url>() else {
        return String::new();
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
            None => return String::new(),
        },
    };
    let host = base.host_str().unwrap_or_default();
    match cookies {
        Ok(cookies) => {
            let header = cookie_header(cookies.iter().map(|c| (c.name(), c.value())));
            if header.is_empty() {
                eprintln!("[oculus] no {host} cookies to save yet");
            }
            header
        }
        Err(e) => {
            eprintln!("[oculus] cookies() failed for {host}: {e}");
            String::new()
        }
    }
}

/// Snapshots the webview's Canvas and Okta cookies into oculus-keyd. Call it
/// only once Canvas has signed the webview in: before that the jar holds the
/// anonymous `canvas_session` Canvas hands every visitor. Blocks on keyd, so
/// not from the main thread. True when the Canvas session was stored.
pub fn save_session_cookie(app: &AppHandle) -> bool {
    session::store_snapshot(
        &Credentialed::at(&crate::library::paths::data_dir()),
        &snapshot(app),
    )
}

/// `save_session_cookie` for a browser tab's page-load callback, which runs
/// on a thread that must not wait on keyd: the jar is read here and handed
/// over on another thread, which then connects the app if the tab was a
/// sign-in.
pub fn save_browser_session(app: &AppHandle) {
    let snapshot = snapshot(app);
    let app = app.clone();
    std::thread::spawn(move || {
        let keyd = Credentialed::at(&crate::library::paths::data_dir());
        if session::store_snapshot(&keyd, &snapshot) {
            tab::confirm_browser_sign_in(&app, &keyd);
        }
    });
}

#[cfg(test)]
mod tests {
    use super::cookie_header;

    #[test]
    fn a_header_keeps_the_first_cookie_of_each_name() {
        assert_eq!(
            cookie_header([
                ("canvas_session", "a"),
                ("_csrf", "b"),
                ("canvas_session", "c")
            ]),
            "canvas_session=a; _csrf=b"
        );
        assert_eq!(cookie_header([]), "");
    }
}
