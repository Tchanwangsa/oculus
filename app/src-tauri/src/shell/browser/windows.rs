//! WebView2 cookie writes are queued by Tauri. Read back on a worker thread
//! before allowing navigation or reporting that sign-out cleared the jar.

use std::time::{Duration, Instant};

use tauri::webview::Cookie;
#[cfg(target_os = "windows")]
use tauri::{AppHandle, Manager, Webview};

use super::seed::sent_to;
use super::CANVAS_HOST;

#[cfg(target_os = "windows")]
static COOKIE_WORK: std::sync::Mutex<()> = std::sync::Mutex::new(());
const COOKIE_WAIT: Duration = Duration::from_secs(5);
const COOKIE_POLL: Duration = Duration::from_millis(25);

type Sessions = Vec<(&'static str, String, bool)>;

trait CookieJar {
    fn cookies(&mut self) -> Result<Vec<Cookie<'static>>, String>;
    fn cookies_for_url(&mut self, url: url::Url) -> Result<Vec<Cookie<'static>>, String>;
    fn set(&mut self, cookie: Cookie<'static>) -> Result<(), String>;
    fn delete(&mut self, cookie: Cookie<'static>) -> Result<(), String>;
}

#[cfg(target_os = "windows")]
impl CookieJar for Webview<tauri::Wry> {
    fn cookies(&mut self) -> Result<Vec<Cookie<'static>>, String> {
        Webview::cookies(self).map_err(|e| e.to_string())
    }

    fn cookies_for_url(&mut self, url: url::Url) -> Result<Vec<Cookie<'static>>, String> {
        Webview::cookies_for_url(self, url).map_err(|e| e.to_string())
    }

    fn set(&mut self, cookie: Cookie<'static>) -> Result<(), String> {
        self.set_cookie(cookie).map_err(|e| e.to_string())
    }

    fn delete(&mut self, cookie: Cookie<'static>) -> Result<(), String> {
        self.delete_cookie(cookie).map_err(|e| e.to_string())
    }
}

fn wait_for(mut confirmed: impl FnMut() -> Result<bool, String>) -> Result<(), String> {
    let deadline = Instant::now() + COOKIE_WAIT;
    loop {
        if confirmed()? {
            return Ok(());
        }
        if Instant::now() >= deadline {
            return Err("WebView2 did not confirm the cookie change".into());
        }
        std::thread::sleep(COOKIE_POLL);
    }
}

fn seed_jar(jar: &mut impl CookieJar, sessions: Sessions) -> Result<(), String> {
    for (host, header, replace) in sessions {
        let url: url::Url = format!("https://{host}/")
            .parse()
            .map_err(|e| format!("{e}"))?;
        let present = jar.cookies_for_url(url.clone())?;
        let mut expected = Vec::new();
        for (name, value) in header.split(';').filter_map(|p| p.trim().split_once('=')) {
            let (name, value) = (name.trim(), value.trim());
            if name.is_empty() || (!replace && present.iter().any(|c| c.name() == name)) {
                continue;
            }
            // Bare headers have no attributes. Preserve a browser cookie's
            // HttpOnly flag; new session cookies are protected, while CSRF
            // and other script-readable cookies remain available to the page.
            let http_only = present
                .iter()
                .find(|c| c.name() == name)
                .and_then(|c| c.http_only())
                .unwrap_or(matches!(name, "canvas_session" | "sid" | "idx"));
            let cookie = Cookie::build((name.to_string(), value.to_string()))
                .domain(host)
                .path("/")
                .secure(true)
                .http_only(http_only)
                .build();
            // AddOrUpdateCookie overwrites the same name/domain/path even
            // when the server's cookie is HttpOnly; no delete is needed.
            jar.set(cookie.clone())?;
            expected.push(cookie);
        }
        if !expected.is_empty() {
            wait_for(|| {
                let actual = jar.cookies_for_url(url.clone())?;
                Ok(expected.iter().all(|wanted| {
                    actual.iter().any(|c| {
                        c.name() == wanted.name()
                            && c.value() == wanted.value()
                            && c.domain() == wanted.domain()
                            && c.path() == wanted.path()
                            && c.http_only() == wanted.http_only()
                            && c.secure() == Some(true)
                    })
                }))
            })?;
        }
    }
    Ok(())
}

fn ours(cookie: &Cookie<'_>) -> bool {
    cookie.domain().is_some_and(|domain| {
        sent_to(domain, CANVAS_HOST) || sent_to(domain, crate::auth::okta::SSO_HOST)
    })
}

fn clear_jar(jar: &mut impl CookieJar) -> Result<(), String> {
    for cookie in jar.cookies()?.into_iter().filter(ours) {
        jar.delete(cookie)?;
    }
    wait_for(|| Ok(!jar.cookies()?.iter().any(ours)))
}

#[cfg(target_os = "windows")]
fn shared_jar(app: &AppHandle) -> Result<Webview<tauri::Wry>, String> {
    app.get_webview(super::MAIN)
        .or_else(|| app.webviews().into_values().next())
        .ok_or_else(|| "no webview available for the shared cookie jar".into())
}

#[cfg(target_os = "windows")]
pub(super) fn seed_sessions(app: &AppHandle, then: impl FnOnce() + Send + 'static) {
    let app = app.clone();
    std::thread::spawn(move || {
        let _work = COOKIE_WORK.lock().unwrap_or_else(|e| e.into_inner());
        let sessions = super::seed::saved_sessions();
        let result = if sessions.is_empty() {
            Ok(())
        } else {
            shared_jar(&app).and_then(|mut jar| seed_jar(&mut jar, sessions))
        };
        if let Err(e) = result {
            // A failed seed must be retried as a replacement, rather than
            // treating keyd's header as already delivered to the browser.
            *super::seed::SSO_SEEDED
                .lock()
                .unwrap_or_else(|e| e.into_inner()) = None;
            eprintln!("[oculus] browser: could not seed WebView2 sessions: {e}");
            return;
        }
        app.run_on_main_thread(then).ok();
    });
}

#[cfg(target_os = "windows")]
pub(super) fn clear_sessions(app: &AppHandle, then: impl FnOnce() + Send + 'static) {
    let app = app.clone();
    std::thread::spawn(move || {
        let _work = COOKIE_WORK.lock().unwrap_or_else(|e| e.into_inner());
        if let Err(e) = shared_jar(&app).and_then(|mut jar| clear_jar(&mut jar)) {
            eprintln!("[oculus] browser: could not clear WebView2 sessions: {e}");
            return;
        }
        *super::seed::SSO_SEEDED
            .lock()
            .unwrap_or_else(|e| e.into_inner()) = None;
        app.run_on_main_thread(then).ok();
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Default)]
    struct QueuedJar {
        held: Vec<Cookie<'static>>,
        sets: Vec<Cookie<'static>>,
        deletes: Vec<Cookie<'static>>,
        reads: usize,
    }

    impl CookieJar for QueuedJar {
        fn cookies(&mut self) -> Result<Vec<Cookie<'static>>, String> {
            self.reads += 1;
            for deleted in self.deletes.drain(..) {
                self.held
                    .retain(|c| c.name() != deleted.name() || c.domain() != deleted.domain());
            }
            for set in self.sets.drain(..) {
                self.held
                    .retain(|c| c.name() != set.name() || c.domain() != set.domain());
                self.held.push(set);
            }
            Ok(self.held.clone())
        }
        fn cookies_for_url(&mut self, url: url::Url) -> Result<Vec<Cookie<'static>>, String> {
            Ok(self
                .cookies()?
                .into_iter()
                .filter(|c| {
                    c.domain()
                        .is_some_and(|d| sent_to(d, url.host_str().unwrap()))
                })
                .collect())
        }
        fn set(&mut self, cookie: Cookie<'static>) -> Result<(), String> {
            self.sets.push(cookie);
            Ok(())
        }
        fn delete(&mut self, cookie: Cookie<'static>) -> Result<(), String> {
            self.deletes.push(cookie);
            Ok(())
        }
    }

    fn cookie(host: &'static str, name: &'static str, value: &'static str) -> Cookie<'static> {
        Cookie::build((name, value))
            .domain(host)
            .path("/")
            .http_only(true)
            .secure(true)
            .build()
    }

    #[test]
    fn queued_writes_are_read_back_for_both_hosts_and_okta_fills_gaps() {
        let sso = crate::auth::okta::SSO_HOST;
        let mut jar = QueuedJar {
            held: vec![
                cookie(CANVAS_HOST, "canvas_session", "anonymous"),
                cookie(sso, "sid", "browser"),
            ],
            ..Default::default()
        };
        seed_jar(
            &mut jar,
            vec![
                (CANVAS_HOST, "canvas_session=saved".into(), true),
                (sso, "sid=old; token=new".into(), false),
            ],
        )
        .unwrap();
        assert!(
            jar.sets.is_empty(),
            "writes must land before navigation continues"
        );
        assert!(
            jar.deletes.is_empty(),
            "HttpOnly updates do not need a delete"
        );
        assert!(jar
            .held
            .iter()
            .any(|c| c.name() == "canvas_session" && c.value() == "saved"));
        assert!(jar
            .held
            .iter()
            .any(|c| c.name() == "sid" && c.value() == "browser"));
        assert!(jar
            .held
            .iter()
            .any(|c| c.name() == "token" && c.value() == "new"));
        seed_jar(&mut jar, vec![(sso, "sid=renewed".into(), true)]).unwrap();
        assert!(jar
            .held
            .iter()
            .any(|c| c.name() == "sid" && c.value() == "renewed"));
        assert_eq!(jar.reads, 6);
    }

    #[test]
    fn canvas_csrf_stays_script_readable_when_a_bare_header_is_seeded() {
        let mut jar = QueuedJar::default();
        seed_jar(
            &mut jar,
            vec![(
                CANVAS_HOST,
                "canvas_session=saved; _csrf_token=csrf".into(),
                true,
            )],
        )
        .unwrap();
        assert_eq!(
            jar.held
                .iter()
                .find(|c| c.name() == "canvas_session")
                .unwrap()
                .http_only(),
            Some(true)
        );
        assert_eq!(
            jar.held
                .iter()
                .find(|c| c.name() == "_csrf_token")
                .unwrap()
                .http_only(),
            Some(false)
        );
    }

    #[test]
    fn sign_out_waits_for_deletes_and_keeps_unrelated_hosts() {
        let mut jar = QueuedJar {
            held: vec![
                cookie(CANVAS_HOST, "canvas_session", "a"),
                cookie("unimelb.edu.au", "shared", "b"),
                cookie(crate::auth::okta::SSO_HOST, "sid", "c"),
                cookie("edstem.org", "ed", "d"),
            ],
            ..Default::default()
        };
        clear_jar(&mut jar).unwrap();
        assert!(jar.deletes.is_empty());
        assert_eq!(jar.held.len(), 1);
        assert_eq!(jar.held[0].name(), "ed");
        assert_eq!(jar.reads, 2);
    }
}
