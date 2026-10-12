#[cfg(target_os = "macos")]
use std::collections::HashSet;
use std::sync::Mutex;
use std::time::Duration;

use tauri::AppHandle;

use super::layout::page;
use super::page::reload_page;
#[cfg(target_os = "macos")]
use super::scope;
use crate::sources::institution;

/// A page on the university's hosts loads only after the saved sessions are
/// in the browser's jar: Canvas, the SSO host, and anything in the domain they
/// share (the university's single sign-on fronts its other services too).
pub(super) fn wants_sessions(url: &url::Url) -> bool {
    let Some(canvas) = institution::canvas_host() else {
        return false;
    };
    let sso = institution::sso_host();
    url.host_str()
        .is_some_and(|h| on_university_hosts(h, &canvas, sso.as_deref()))
}

pub(super) fn on_university_hosts(host: &str, canvas: &str, sso: Option<&str>) -> bool {
    host == canvas
        || sso == Some(host)
        || institution::institution_domain(canvas, sso).is_some_and(|d| sent_to(&d, host))
}

/// A fingerprint of the SSO header last seeded. While the app runs the
/// browser's own SSO cookies are the freshest, so they are replaced only by a
/// header oculus-keyd now holds that differs (a headless sign-in); otherwise a
/// seed fills gaps.
#[cfg(any(target_os = "macos", target_os = "windows"))]
pub(super) static SSO_SEEDED: Mutex<Option<u64>> = Mutex::new(None);

/// Whether the SSO host's `now` header replaces the jar's cookies, given the
/// fingerprint `seeded` of the last one seeded; records `now` as seeded. The
/// first seed of a run replaces when keyd holds a header at all.
#[cfg(any(target_os = "macos", target_os = "windows"))]
pub(super) fn sso_changed(seeded: &mut Option<u64>, now: Option<&str>) -> bool {
    use std::hash::{Hash, Hasher};

    let now = now.map(|header| {
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        header.hash(&mut hasher);
        hasher.finish()
    });
    let changed = *seeded != now;
    *seeded = now;
    changed
}

/// Each stored session as (host, bare `name=value; …` header, whether it
/// replaces the jar's same-named cookies), for the profile's hosts. The
/// scraper keeps Canvas's fresh, so it always replaces. Asks oculus-keyd, so
/// not from the main thread; an absent keyd, one with no session, or no
/// profile seeds nothing.
#[cfg(any(target_os = "macos", target_os = "windows"))]
pub(super) fn saved_sessions() -> Vec<(String, String, bool)> {
    let Some(inst) = institution::current() else {
        return Vec::new();
    };
    let Some(canvas) = institution::host_of(&inst.canvas) else {
        return Vec::new();
    };
    let keyd = crate::providers::credentials::Credentialed::at(&crate::library::paths::data_dir());
    sessions_from(&keyd, &SSO_SEEDED, &canvas, inst.session_sso_host())
}

/// `saved_sessions` for the keyd `keyd` reaches and the hosts `canvas` and
/// `sso`, comparing the SSO header with the fingerprint in `sso_seeded`.
#[cfg(any(target_os = "macos", target_os = "windows"))]
pub(super) fn sessions_from(
    keyd: &crate::providers::credentials::Credentialed,
    sso_seeded: &Mutex<Option<u64>>,
    canvas: &str,
    sso: Option<&str>,
) -> Vec<(String, String, bool)> {
    let cookies = match keyd.session_get() {
        Ok(cookies) => cookies,
        Err(e) => {
            crate::auth::report("could not read the stored sessions to seed the browser", &e);
            return Vec::new();
        }
    };
    let replace_sso = {
        let mut seeded = sso_seeded.lock().unwrap_or_else(|e| e.into_inner());
        sso_changed(&mut seeded, cookies.sso.as_deref())
    };
    let sso = sso.map(|host| {
        (
            host.to_string(),
            cookies.sso.unwrap_or_default(),
            replace_sso,
        )
    });
    std::iter::once((canvas.to_string(), cookies.canvas.unwrap_or_default(), true))
        .chain(sso)
        .filter(|(_, header, _)| !header.trim().is_empty())
        .collect()
}

/// Copies the stored Canvas and SSO sessions into WebKit's shared jar, each
/// scoped to its host, then runs `then` on the main thread. Loads go in
/// `then`: `setCookies` is async, and a request sent before it lands meets
/// Canvas anonymous. The sessions are fetched from oculus-keyd on a thread of
/// its own, so a keychain prompt there never holds up the UI.
///
/// Same-named cookies set on those hosts themselves are deleted first. WebKit
/// will not let an API-set cookie replace an HttpOnly one a server set, so
/// once Canvas hands out an anonymous `canvas_session`, a plain re-set is
/// silently dropped. A name the jar holds on a parent domain is left alone and
/// not seeded: a host copy would shadow the parent's cookie, which may be the
/// student's own (`scope`).
#[cfg(target_os = "macos")]
pub fn seed_sessions(app: &AppHandle, then: impl FnOnce() + Send + 'static) {
    let app = app.clone();
    std::thread::spawn(move || {
        let sessions = saved_sessions();
        seed_webkit(&app, sessions, then);
    });
}

/// `seed_sessions` once the sessions are in hand.
#[cfg(target_os = "macos")]
fn seed_webkit(
    app: &AppHandle,
    sessions: Vec<(String, String, bool)>,
    then: impl FnOnce() + Send + 'static,
) {
    use std::cell::Cell;
    use std::ptr::NonNull;
    use std::rc::Rc;

    use block2::RcBlock;
    use objc2::runtime::AnyObject;
    use objc2::MainThreadMarker;
    use objc2_foundation::{
        NSArray, NSDictionary, NSHTTPCookie, NSHTTPCookieDomain, NSHTTPCookieName,
        NSHTTPCookiePath, NSHTTPCookieSecure, NSHTTPCookieValue, NSString,
    };
    use objc2_web_kit::WKWebsiteDataStore;

    // Off the delegate callback that fires it, as `on_new_window` does.
    let then: Box<dyn FnOnce() + Send> = Box::new(then);
    let finish_app = app.clone();
    let finish = move || {
        std::thread::spawn(move || {
            finish_app.run_on_main_thread(then).ok();
        });
    };

    if sessions.is_empty() {
        finish();
        return;
    }

    // `defaultDataStore` is main-thread-only, and so is everything downstream.
    let result = app.run_on_main_thread(move || {
        let Some(mtm) = MainThreadMarker::new() else {
            finish();
            return;
        };
        let path = NSString::from_str("/");
        let secure = NSString::from_str("TRUE");

        // (host, name, replaces, cookie)
        let mut cookies = Vec::new();
        for (host, header, replace) in &sessions {
            let domain = NSString::from_str(host);
            for (name, value) in header
                .split(';')
                .filter_map(|pair| pair.trim().split_once('='))
            {
                let (name, value) = (name.trim(), value.trim());
                let ns_name = NSString::from_str(name);
                let ns_value = NSString::from_str(value);
                let keys: [&NSString; 5] = unsafe {
                    [
                        NSHTTPCookieName,
                        NSHTTPCookieValue,
                        NSHTTPCookieDomain,
                        NSHTTPCookiePath,
                        NSHTTPCookieSecure,
                    ]
                };
                let values: [&AnyObject; 5] = [&ns_name, &ns_value, &domain, &path, &secure];
                let props = NSDictionary::from_slices(&keys, &values);
                if let Some(cookie) = unsafe { NSHTTPCookie::cookieWithProperties(&props) } {
                    cookies.push((host.to_string(), name.to_string(), *replace, cookie));
                }
            }
        }
        if cookies.is_empty() {
            finish();
            return;
        }
        let store = unsafe { WKWebsiteDataStore::defaultDataStore(mtm).httpCookieStore() };

        let finish = Rc::new(Cell::new(Some(finish)));
        let get_store = store.clone();
        let got_all = RcBlock::new(move |all: NonNull<NSArray<NSHTTPCookie>>| {
            let all = unsafe { all.as_ref() };
            let key = |c: &NSHTTPCookie| {
                let domain = c.domain().to_string();
                (
                    domain.trim_start_matches('.').to_string(),
                    c.name().to_string(),
                )
            };
            let present: HashSet<_> = all.iter().map(|c| key(&c)).collect();
            let replacing: HashSet<_> = cookies
                .iter()
                .filter(|(_, _, replace, _)| *replace)
                .map(|(host, name, _, _)| (host.clone(), name.clone()))
                .collect();
            let stale: Vec<_> = all
                .iter()
                .filter(|c| {
                    let (domain, name) = key(c);
                    replacing.iter().any(|(host, replacing_name)| {
                        name == *replacing_name && scope::on_host(&domain, host)
                    })
                })
                .collect();
            let from_parent: HashSet<_> = cookies
                .iter()
                .filter(|(host, name, _, _)| {
                    all.iter().any(|c| {
                        c.name().to_string() == *name
                            && scope::at_parent(&c.domain().to_string(), host)
                    })
                })
                .map(|(host, name, _, _)| (host.clone(), name.clone()))
                .collect();
            let fresh: Vec<_> = cookies
                .iter()
                .filter(|(host, name, replace, _)| {
                    let at = (host.clone(), name.clone());
                    !from_parent.contains(&at) && (*replace || !present.contains(&at))
                })
                .map(|(_, _, _, cookie)| cookie.clone())
                .collect();
            let count = fresh.len();
            let seeded: Vec<_> = fresh
                .iter()
                .map(|c| {
                    (
                        c.domain().to_string(),
                        c.name().to_string(),
                        c.value().to_string(),
                    )
                })
                .collect();
            let fresh = NSArray::from_retained_slice(&fresh);

            // Never pass a nil completion handler: WebKit invokes it anyway
            // and the app segfaults later, far from here.
            let finish = finish.clone();
            let done = RcBlock::new(move || {
                if let Some(finish) = finish.take() {
                    crate::auth::remember_seeded_cookies(seeded.clone());
                    elog!("[oculus] browser: seeded {count} session cookies into WebKit");
                    finish();
                }
            });
            let set_store = store.clone();
            let set_all = Rc::new(move || unsafe {
                set_store.setCookies_completionHandler(&fresh, Some(&done));
            });
            if stale.is_empty() {
                set_all();
                return;
            }
            // Set only once every delete has landed.
            let pending = Rc::new(Cell::new(stale.len()));
            for cookie in &stale {
                let pending = pending.clone();
                let set_all = set_all.clone();
                let deleted = RcBlock::new(move || {
                    pending.set(pending.get() - 1);
                    if pending.get() == 0 {
                        set_all();
                    }
                });
                unsafe { store.deleteCookie_completionHandler(cookie, Some(&deleted)) };
            }
        });
        unsafe { get_store.getAllCookies(&got_all) };
    });
    if result.is_err() {
        elog!("[oculus] browser: could not reach the main thread to seed cookies");
    }
}

/// Okta's entry point for a SAML app (Canvas, a library, …). It renders the
/// sign-in form when there is no Okta session, an auto-posting form when
/// there is one. Only an Okta university has one.
pub(super) fn is_sso_app_entry(url: &url::Url) -> bool {
    institution::is_okta()
        && url
            .host_str()
            .is_some_and(|h| institution::sso_host().as_deref() == Some(h))
        && url.path().starts_with("/app/")
        && url.path().ends_with("/sso/saml")
}

/// A page reached Okta's sign-in for a university app: if the browser holds no
/// live Okta session, sign in headlessly (which saves Okta's cookies), seed
/// them and reload. Asks Okta, not the page, whether a session exists.
pub(super) fn recover_sso(app: &AppHandle, id: u32) {
    let Some(webview) = page(app, id) else {
        return;
    };
    let app = app.clone();
    std::thread::spawn(move || {
        let Some(sso_host) = institution::sso_host() else {
            return;
        };
        let Ok(sso) = format!("https://{sso_host}").parse::<url::Url>() else {
            return;
        };
        let header = webview
            .cookies_for_url(sso)
            .map(|cookies| {
                cookies
                    .iter()
                    .map(|c| format!("{}={}", c.name(), c.value()))
                    .collect::<Vec<_>>()
                    .join("; ")
            })
            .unwrap_or_default();
        let me = format!("https://{sso_host}/api/v1/sessions/me");
        let mut probe = ureq::get(&me)
            .timeout(Duration::from_secs(20))
            .set("Accept", "application/json");
        if !header.is_empty() {
            probe = probe.set("Cookie", &header);
        }
        match probe.call() {
            Ok(_) => return,
            Err(ureq::Error::Status(404 | 401 | 403, _)) => {}
            // Unreachable: no verdict, so no sign-in attempt.
            Err(_) => return,
        }

        // The attempt guard in `okta::sign_in` keeps a session Okta will not
        // honour from looping sign-ins.
        elog!("[oculus] browser: tab {id} reached Okta sign-in with no session; signing in");
        if !crate::auth::okta::try_auto_recover(&app, crate::auth::okta::Trigger::Browser) {
            return;
        }
        let reload_app = app.clone();
        seed_sessions(&app, move || reload_page(&reload_app, id, false));
    });
}

#[cfg(target_os = "windows")]
pub fn seed_sessions(app: &AppHandle, then: impl FnOnce() + Send + 'static) {
    super::windows::seed_sessions(app, then);
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
pub fn seed_sessions(_app: &AppHandle, then: impl FnOnce() + Send + 'static) {
    then();
}

/// Whether a cookie scoped to `domain` is sent to `host`, as
/// `cookies_for_url` decides it.
pub(super) fn sent_to(domain: &str, host: &str) -> bool {
    let domain = domain.trim_start_matches('.');
    host == domain || host.ends_with(&format!(".{domain}"))
}

/// Deletes every cookie in WebKit's shared jar that is the university's
/// session at Canvas or its SSO host (`scope::belongs`), then runs `then`.
/// Sign-out calls it: otherwise a browser tab is still signed in and a Canvas
/// page saves the session straight back to disk. A shared provider's cookies
/// (`Institution::session_sso_host`) and other parent domains stay. `then` is
/// dropped uncalled when the main thread cannot be reached.
#[cfg(target_os = "macos")]
pub fn clear_sessions(app: &AppHandle, then: impl FnOnce() + Send + 'static) {
    use std::cell::Cell;
    use std::ptr::NonNull;
    use std::rc::Rc;

    use block2::RcBlock;
    use objc2::MainThreadMarker;
    use objc2_foundation::{NSArray, NSHTTPCookie};
    use objc2_web_kit::WKWebsiteDataStore;

    let then: Box<dyn FnOnce() + Send> = Box::new(then);
    let (hosts, university) = session_hosts();
    let result = app.run_on_main_thread(move || {
        let Some(mtm) = MainThreadMarker::new() else {
            return;
        };
        let store = unsafe { WKWebsiteDataStore::defaultDataStore(mtm).httpCookieStore() };
        let then = Rc::new(Cell::new(Some(then)));
        let get_store = store.clone();
        let got_all = RcBlock::new(move |all: NonNull<NSArray<NSHTTPCookie>>| {
            let all = unsafe { all.as_ref() };
            let ours: Vec<_> = all
                .iter()
                .filter(|c| {
                    let domain = c.domain().to_string();
                    hosts
                        .iter()
                        .any(|host| scope::belongs(&domain, host, university.as_deref()))
                })
                .collect();
            let count = ours.len();
            let finish = {
                let then = then.clone();
                Rc::new(move || {
                    if let Some(then) = then.take() {
                        elog!(
                            "[oculus] browser: cleared {count} Canvas and SSO cookies from WebKit"
                        );
                        then();
                    }
                })
            };
            if ours.is_empty() {
                finish();
                return;
            }
            let pending = Rc::new(Cell::new(count));
            for cookie in &ours {
                let pending = pending.clone();
                let finish = finish.clone();
                let deleted = RcBlock::new(move || {
                    pending.set(pending.get() - 1);
                    if pending.get() == 0 {
                        finish();
                    }
                });
                unsafe { store.deleteCookie_completionHandler(cookie, Some(&deleted)) };
            }
        });
        unsafe { get_store.getAllCookies(&got_all) };
    });
    if result.is_err() {
        elog!("[oculus] browser: could not reach the main thread to clear cookies");
    }
}

/// The hosts whose session sign-out clears (Canvas's, and the SSO host's
/// unless it is a shared provider) and the university's own domain.
#[cfg(any(target_os = "macos", target_os = "windows"))]
pub(super) fn session_hosts() -> (Vec<String>, Option<String>) {
    let Some(inst) = institution::current() else {
        return (Vec::new(), None);
    };
    let hosts = institution::host_of(&inst.canvas)
        .into_iter()
        .chain(inst.session_sso_host().map(str::to_string))
        .collect();
    (hosts, inst.session_domain())
}

#[cfg(target_os = "windows")]
pub fn clear_sessions(app: &AppHandle, then: impl FnOnce() + Send + 'static) {
    super::windows::clear_sessions(app, then);
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
pub fn clear_sessions(_app: &AppHandle, then: impl FnOnce() + Send + 'static) {
    then();
}
