//! Seeding WebKit's cookie jar with the stored Canvas and Okta sessions, and
//! clearing it on sign-out.

use super::*;

/// UniMelb's Okta sign-on fronts every service under this domain, so a page
/// here loads only after the saved sessions are in WebKit's jar.
pub(super) fn wants_sessions(url: &url::Url) -> bool {
    url.host_str()
        .is_some_and(|h| h == "unimelb.edu.au" || h.ends_with(".unimelb.edu.au"))
}

/// A fingerprint of the Okta header last seeded. While the app runs the
/// browser's own Okta cookies are the freshest, so they are replaced only by a
/// header oculus-keyd now holds that differs (a headless sign-in); otherwise a
/// seed fills gaps.
#[cfg(target_os = "macos")]
pub(super) static SSO_SEEDED: Mutex<Option<u64>> = Mutex::new(None);

/// Whether Okta's `now` header replaces the jar's cookies, given the
/// fingerprint `seeded` of the last one seeded; records `now` as seeded. The
/// first seed of a run replaces when keyd holds a header at all.
#[cfg(target_os = "macos")]
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
/// replaces the jar's same-named cookies). The scraper keeps Canvas's fresh,
/// so it always replaces. Asks oculus-keyd, so not from the main thread; an
/// absent keyd or one with no session seeds nothing.
#[cfg(target_os = "macos")]
pub(super) fn saved_sessions() -> Vec<(&'static str, String, bool)> {
    let keyd = crate::credentials::Credentialed::at(&crate::paths::data_dir());
    sessions_from(&keyd, &SSO_SEEDED)
}

/// `saved_sessions` for the keyd `keyd` reaches, comparing Okta's header with
/// the fingerprint in `sso_seeded`.
#[cfg(target_os = "macos")]
pub(super) fn sessions_from(
    keyd: &crate::credentials::Credentialed,
    sso_seeded: &Mutex<Option<u64>>,
) -> Vec<(&'static str, String, bool)> {
    let cookies = match keyd.session_get() {
        Ok(cookies) => cookies,
        Err(e) => {
            crate::auth::report("could not read the stored sessions to seed WebKit", &e);
            return Vec::new();
        }
    };
    let replace_sso = {
        let mut seeded = sso_seeded.lock().unwrap_or_else(|e| e.into_inner());
        sso_changed(&mut seeded, cookies.sso.as_deref())
    };
    [
        (CANVAS_HOST, cookies.canvas.unwrap_or_default(), true),
        (
            crate::okta::SSO_HOST,
            cookies.sso.unwrap_or_default(),
            replace_sso,
        ),
    ]
    .into_iter()
    .filter(|(_, header, _)| !header.trim().is_empty())
    .collect()
}

/// Copies the stored Canvas and Okta sessions into WebKit's shared jar, each
/// scoped to its host, then runs `then` on the main thread. Loads go in
/// `then`: `setCookies` is async, and a request sent before it lands meets
/// Canvas anonymous. The sessions are fetched from oculus-keyd on a thread of
/// its own, so a keychain prompt there never holds up the UI.
///
/// Same-named cookies on those hosts are deleted first. WebKit will not let an
/// API-set cookie replace an HttpOnly one a server set, so once Canvas hands
/// out an anonymous `canvas_session`, a plain re-set is silently dropped.
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
pub(super) fn seed_webkit(
    app: &AppHandle,
    sessions: Vec<(&'static str, String, bool)>,
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
            let stale: Vec<_> = all.iter().filter(|c| replacing.contains(&key(c))).collect();
            let fresh: Vec<_> = cookies
                .iter()
                .filter(|(host, name, replace, _)| {
                    *replace || !present.contains(&(host.clone(), name.clone()))
                })
                .map(|(_, _, _, cookie)| cookie.clone())
                .collect();
            let count = fresh.len();
            let fresh = NSArray::from_retained_slice(&fresh);

            // Never pass a nil completion handler: WebKit invokes it anyway
            // and the app segfaults later, far from here.
            let finish = finish.clone();
            let done = RcBlock::new(move || {
                if let Some(finish) = finish.take() {
                    eprintln!("[oculus] browser: seeded {count} UniMelb cookies into WebKit");
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
        eprintln!("[oculus] browser: could not reach the main thread to seed cookies");
    }
}

/// Okta's entry point for a SAML app (Canvas, DiBS, …). It renders the
/// sign-in form when there is no Okta session, an auto-posting form when
/// there is one.
pub(super) fn is_sso_app_entry(url: &url::Url) -> bool {
    url.host_str() == Some(crate::okta::SSO_HOST)
        && url.path().starts_with("/app/")
        && url.path().ends_with("/sso/saml")
}

/// A page reached Okta's sign-in for a UniMelb app: if the browser holds no
/// live Okta session, sign in headlessly (which saves Okta's cookies), seed
/// them and reload. Asks Okta, not the page, whether a session exists.
pub(super) fn recover_sso(app: &AppHandle, id: u32) {
    let Some(webview) = page(app, id) else {
        return;
    };
    let app = app.clone();
    std::thread::spawn(move || {
        let Ok(sso) = format!("https://{}", crate::okta::SSO_HOST).parse::<url::Url>() else {
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
        let me = format!("https://{}/api/v1/sessions/me", crate::okta::SSO_HOST);
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
        eprintln!("[oculus] browser: tab {id} reached Okta sign-in with no session; signing in");
        if !crate::okta::try_auto_recover(&app, crate::okta::Trigger::Browser) {
            return;
        }
        let reload_app = app.clone();
        seed_sessions(&app, move || reload_page(&reload_app, id, false));
    });
}

#[cfg(not(target_os = "macos"))]
pub fn seed_sessions(_app: &AppHandle, then: impl FnOnce() + Send + 'static) {
    then();
}

/// Whether a cookie scoped to `domain` is sent to `host`, as
/// `cookies_for_url` decides it.
pub(super) fn sent_to(domain: &str, host: &str) -> bool {
    let domain = domain.trim_start_matches('.');
    host == domain || host.ends_with(&format!(".{domain}"))
}

/// Deletes every cookie WebKit's shared jar would send to Canvas or Okta, then
/// runs `then`. Sign-out calls it: otherwise a browser tab is still signed in
/// and a Canvas page saves the session straight back to disk. `then` is
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
                    sent_to(&domain, CANVAS_HOST) || sent_to(&domain, crate::okta::SSO_HOST)
                })
                .collect();
            let count = ours.len();
            let finish = {
                let then = then.clone();
                Rc::new(move || {
                    if let Some(then) = then.take() {
                        eprintln!(
                            "[oculus] browser: cleared {count} Canvas and Okta cookies from WebKit"
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
        eprintln!("[oculus] browser: could not reach the main thread to clear cookies");
    }
}

#[cfg(not(target_os = "macos"))]
pub fn clear_sessions(_app: &AppHandle, then: impl FnOnce() + Send + 'static) {
    then();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sign_out_clears_what_cookies_for_url_would_send() {
        assert!(sent_to("canvas.lms.unimelb.edu.au", CANVAS_HOST));
        assert!(sent_to(".unimelb.edu.au", CANVAS_HOST));
        assert!(sent_to(".sso.unimelb.edu.au", crate::okta::SSO_HOST));
        assert!(!sent_to("library.unimelb.edu.au", CANVAS_HOST));
        assert!(!sent_to("lms.unimelb.edu.au.evil.com", CANVAS_HOST));
        assert!(!sent_to("edstem.org", crate::okta::SSO_HOST));
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn okta_replaces_the_jar_only_when_the_stored_header_changed() {
        let mut seeded = None;
        assert!(
            !sso_changed(&mut seeded, None),
            "nothing stored, nothing to replace"
        );
        assert!(
            sso_changed(&mut seeded, Some("sid=a")),
            "the first seed of a run"
        );
        assert!(!sso_changed(&mut seeded, Some("sid=a")), "the same header");
        assert!(
            sso_changed(&mut seeded, Some("sid=b")),
            "a headless sign-in wrote a new one"
        );
        assert!(sso_changed(&mut seeded, None), "the session was dropped");
        assert!(!sso_changed(&mut seeded, None));
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn sessions_come_from_keyd_and_a_missing_keyd_or_session_seeds_nothing() {
        use crate::test_support::{FakeKeyd, Scratch};
        use serde_json::json;

        let dir = Scratch::new("seed-sessions");
        let held = std::sync::Arc::new(Mutex::new(
            json!({"canvas": "canvas_session=a", "sso": "sid=1"}),
        ));
        let served = held.clone();
        let keyd = FakeKeyd::start(&dir, move |_, _| (served.lock().unwrap().clone(), vec![]));
        let client = crate::credentials::Credentialed::at(&dir);
        let seeded = Mutex::new(None);

        let sessions = sessions_from(&client, &seeded);
        assert_eq!(
            sessions,
            vec![
                (CANVAS_HOST, "canvas_session=a".to_string(), true),
                (crate::okta::SSO_HOST, "sid=1".to_string(), true),
            ]
        );
        let sessions = sessions_from(&client, &seeded);
        assert!(sessions[0].2, "Canvas always replaces");
        assert!(!sessions[1].2, "Okta was seeded already");

        *held.lock().unwrap() = json!({"canvas": "canvas_session=a", "sso": null});
        let sessions = sessions_from(&client, &seeded);
        assert_eq!(sessions.len(), 1, "no Okta session, none seeded");
        assert_eq!(keyd.ops(), ["session_get", "session_get", "session_get"]);

        let empty = Scratch::new("seed-sessions-absent");
        let none = Mutex::new(None);
        assert!(sessions_from(&crate::credentials::Credentialed::at(&empty), &none).is_empty());
        assert_eq!(
            *none.lock().unwrap(),
            None,
            "a keyd that cannot be asked changes nothing"
        );
    }
}
