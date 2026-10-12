//! Reading the Canvas and SSO cookies out of a live webview and handing them
//! to oculus-keyd, which holds the session from then on. The Canvas session
//! cookie is HttpOnly and session-scoped, so the webview's jar is the only
//! place a sign-in made there can be read from.

use keyd_core::client::SessionKind as Kind;
use std::collections::HashMap;
use std::sync::{LazyLock, Mutex};
use tauri::webview::Cookie;
use tauri::{AppHandle, Manager};

use super::{session, tab};
use crate::providers::credentials::Credentialed;
use crate::shell::browser::scope;

type SeededCookies = HashMap<(String, String), String>;
static SEEDED: LazyLock<Mutex<SeededCookies>> = LazyLock::new(|| Mutex::new(HashMap::new()));

/// Track the host copies written by seeding so a server's rotated
/// parent-domain cookie wins over an unchanged copy of the saved session.
pub(crate) fn remember_seeded_cookies(cookies: Vec<(String, String, String)>) {
    let mut seeded = SEEDED.lock().unwrap_or_else(|e| e.into_inner());
    for (domain, name, value) in cookies {
        seeded.insert((domain.trim_start_matches('.').to_string(), name), value);
    }
}

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

/// Wry's macOS `cookies_for_url` compares domains for equality, omitting
/// parent-domain cookies. Read the jar and scope it ourselves before saving:
/// the host's own cookies, and a parent domain's only when it is the
/// university's (`university`, `shell::browser::scope`).
fn scoped_header(
    cookies: &[Cookie<'_>],
    url: &url::Url,
    university: Option<&str>,
    seeded: &SeededCookies,
) -> String {
    let Some(host) = url.host_str() else {
        return String::new();
    };
    let mut scoped: Vec<_> = cookies
        .iter()
        .filter(|cookie| {
            cookie
                .domain()
                .is_some_and(|domain| scope::belongs(domain, host, university))
                && (!cookie.secure().unwrap_or(false) || url.scheme() == "https")
        })
        .collect();
    scoped.sort_by_key(|cookie| {
        let domain = cookie.domain().unwrap_or("").trim_start_matches('.');
        let is_seeded = seeded
            .get(&(domain.to_string(), cookie.name().to_string()))
            .is_some_and(|value| value == cookie.value());
        (is_seeded, std::cmp::Reverse(domain.len()))
    });
    cookie_header(scoped.into_iter().map(|c| (c.name(), c.value())))
}

/// The Canvas and SSO cookies from a live webview (HttpOnly included), as
/// the sessions to store: the profile's hosts, whatever the university's IdP,
/// except a shared provider's (`Institution::session_sso_host`). An empty
/// header means that host had none yet. Only reads the jar; no socket call.
fn snapshot(app: &AppHandle) -> Vec<(Kind, String)> {
    let Some(inst) = crate::sources::institution::current() else {
        return Vec::new();
    };
    let university = inst.session_domain();
    let university = university.as_deref();
    let mut sessions = vec![(Kind::Canvas, webview_header(app, &inst.canvas, university))];
    if let Some(sso) = inst.session_sso_host() {
        let header = webview_header(app, &format!("https://{sso}"), university);
        sessions.push((Kind::Sso, header));
    }
    sessions
}

/// Scoped by domain after reading the jar: every webview shares one jar
/// (the default profile on Windows; `data_directory` is a no-op on WKWebView).
fn webview_header(app: &AppHandle, base: &str, university: Option<&str>) -> String {
    let Ok(base) = base.parse::<url::Url>() else {
        return String::new();
    };
    // Same jar either way; fall back to an in-app browser tab when the login
    // window is gone.
    let cookies = match app.get_webview_window("canvas-auth") {
        Some(win) => win.cookies(),
        None => match app
            .webviews()
            .into_iter()
            .find(|(label, _)| label.starts_with(crate::shell::browser::LABEL_PREFIX))
        {
            Some((_, webview)) => webview.cookies(),
            None => return String::new(),
        },
    };
    let host = base.host_str().unwrap_or_default();
    match cookies {
        Ok(cookies) => {
            let seeded = SEEDED.lock().unwrap_or_else(|e| e.into_inner());
            let header = scoped_header(&cookies, &base, university, &seeded);
            if header.is_empty() {
                elog!("[oculus] no {host} cookies to save yet");
            }
            header
        }
        Err(e) => {
            elog!("[oculus] cookies() failed for {host}: {e}");
            String::new()
        }
    }
}

/// Snapshots the webview's Canvas and SSO cookies into oculus-keyd. Call it
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
/// on a thread that must not wait on keyd: the jar is read and handed
/// over on another thread, which then connects the app if the tab was a
/// sign-in.
pub fn save_browser_session(app: &AppHandle) {
    let app = app.clone();
    std::thread::spawn(move || {
        let snapshot = snapshot(&app);
        let keyd = Credentialed::at(&crate::library::paths::data_dir());
        if session::store_snapshot(&keyd, &snapshot) {
            tab::confirm_browser_sign_in(&app, &keyd);
        }
    });
}

#[cfg(test)]
mod tests {
    use super::{cookie_header, scoped_header, SeededCookies};
    use tauri::webview::Cookie;

    /// A university whose Canvas and SSO share `rmit.edu.au`.
    const UNI: Option<&str> = Some("rmit.edu.au");

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

    #[test]
    fn snapshots_take_the_universitys_parent_domain_but_no_one_elses() {
        let cookies = [
            Cookie::build(("canvas_session", "canvas"))
                .domain("canvas.lms.unimelb.edu.au")
                .secure(true)
                .build(),
            Cookie::build(("shared", "university"))
                .domain(".unimelb.edu.au")
                .build(),
            Cookie::build(("SID", "google"))
                .domain(".google.com")
                .build(),
        ];
        let canvas = "https://canvas.lms.unimelb.edu.au/".parse().unwrap();
        assert_eq!(
            scoped_header(
                &cookies,
                &canvas,
                Some("unimelb.edu.au"),
                &SeededCookies::new()
            ),
            "canvas_session=canvas; shared=university"
        );
        let google = "https://accounts.google.com/".parse().unwrap();
        assert_eq!(
            scoped_header(&cookies, &google, None, &SeededCookies::new()),
            ""
        );
    }

    #[test]
    fn snapshots_exclude_shared_parent_domains_and_other_hosts() {
        let cookies = [
            Cookie::build(("canvas_session", "canvas"))
                .domain("rmit.instructure.com")
                .secure(true)
                .build(),
            Cookie::build(("shared", "parent"))
                .domain(".instructure.com")
                .build(),
            Cookie::build(("other", "sibling"))
                .domain("other.instructure.com")
                .build(),
            Cookie::build(("lookalike", "foreign"))
                .domain("mit.instructure.com")
                .build(),
            Cookie::build(("idp", "sso"))
                .domain(".rmit.edu.au")
                .secure(true)
                .build(),
        ];
        let canvas = "https://rmit.instructure.com/".parse().unwrap();
        assert_eq!(
            scoped_header(&cookies, &canvas, None, &SeededCookies::new()),
            "canvas_session=canvas"
        );
        let sso = "https://myapps.rmit.edu.au/".parse().unwrap();
        assert_eq!(
            scoped_header(&cookies, &sso, Some("rmit.edu.au"), &SeededCookies::new()),
            "idp=sso"
        );
        assert_eq!(
            scoped_header(&cookies, &sso, None, &SeededCookies::new()),
            ""
        );
        let http = "http://rmit.instructure.com/".parse().unwrap();
        assert_eq!(
            scoped_header(&cookies, &http, None, &SeededCookies::new()),
            ""
        );
    }

    #[test]
    fn host_copy_wins_regardless_of_jar_order() {
        let parent = Cookie::build(("sid", "parent"))
            .domain(".rmit.edu.au")
            .build();
        let host = Cookie::build(("sid", "host"))
            .domain("myapps.rmit.edu.au")
            .build();
        let url = "https://myapps.rmit.edu.au/".parse().unwrap();
        assert_eq!(
            scoped_header(
                &[parent.clone(), host.clone()],
                &url,
                UNI,
                &SeededCookies::new()
            ),
            "sid=host"
        );
        assert_eq!(
            scoped_header(&[host, parent], &url, UNI, &SeededCookies::new()),
            "sid=host"
        );
    }

    #[test]
    fn a_rotated_server_cookie_wins_over_an_unchanged_seed() {
        let parent = Cookie::build(("sid", "rotated"))
            .domain(".rmit.edu.au")
            .build();
        let host = Cookie::build(("sid", "saved"))
            .domain("myapps.rmit.edu.au")
            .build();
        let seeded =
            SeededCookies::from([(("myapps.rmit.edu.au".into(), "sid".into()), "saved".into())]);
        let url = "https://myapps.rmit.edu.au/".parse().unwrap();
        assert_eq!(
            scoped_header(&[host.clone(), parent.clone()], &url, UNI, &seeded),
            "sid=rotated"
        );
        assert_eq!(
            scoped_header(&[parent.clone(), host.clone()], &url, UNI, &seeded),
            "sid=rotated"
        );
        let mut fresh_host = host;
        fresh_host.set_value("new-host");
        assert_eq!(
            scoped_header(&[parent, fresh_host], &url, UNI, &seeded),
            "sid=new-host"
        );
    }
}
