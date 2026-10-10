//! Tauri has no API for history state or find, so these go through the
//! WKWebView. `with_webview` returns nothing, so each answer is written into
//! the tab (or an event) and broadcast from inside the closure.

use serde::Serialize;
use tauri::{AppHandle, Emitter, EventTarget};

use super::layout::page;
use super::{broadcast, with_state, MAIN};

#[derive(Clone, Serialize)]
struct FindResult {
    id: u32,
    query: String,
    found: bool,
}

#[cfg(target_os = "macos")]
pub(super) fn refresh_nav(app: &AppHandle, id: u32) {
    use objc2_web_kit::WKWebView;

    let Some(page) = page(app, id) else {
        return;
    };
    let app = app.clone();
    page.with_webview(move |platform| {
        let ptr = platform.inner() as *mut WKWebView;
        if ptr.is_null() {
            return;
        }
        let (back, forward) = unsafe {
            let view = &*ptr;
            (view.canGoBack(), view.canGoForward())
        };
        let changed = with_state(&app, |s| {
            let Some(tab) = s.tabs.iter_mut().find(|t| t.id == id) else {
                return false;
            };
            if tab.can_back == back && tab.can_forward == forward {
                return false;
            }
            tab.can_back = back;
            tab.can_forward = forward;
            true
        });
        if changed {
            broadcast(&app);
        }
    })
    .ok();
}

/// Elsewhere there is nothing to ask, so both arrows stay live.
#[cfg(not(target_os = "macos"))]
pub(super) fn refresh_nav(app: &AppHandle, id: u32) {
    let changed = with_state(app, |s| {
        let Some(tab) = s.tabs.iter_mut().find(|t| t.id == id) else {
            return false;
        };
        let changed = !tab.can_back || !tab.can_forward;
        tab.can_back = true;
        tab.can_forward = true;
        changed
    });
    if changed {
        broadcast(app);
    }
}

/// The native call, not `history.go`: it works on pages that replaced
/// `history` or run no script.
#[cfg(target_os = "macos")]
pub(super) fn go_history(app: &AppHandle, id: u32, delta: i32) {
    use objc2_web_kit::WKWebView;

    let Some(page) = page(app, id) else {
        return;
    };
    page.with_webview(move |platform| {
        let ptr = platform.inner() as *mut WKWebView;
        if ptr.is_null() {
            return;
        }
        let view = unsafe { &*ptr };
        for _ in 0..delta.unsigned_abs() {
            unsafe {
                if delta < 0 {
                    view.goBack();
                } else {
                    view.goForward();
                }
            }
        }
    })
    .ok();
}

#[cfg(not(target_os = "macos"))]
pub(super) fn go_history(app: &AppHandle, id: u32, delta: i32) {
    if let Some(webview) = page(app, id) {
        webview.eval(&format!("history.go({delta})")).ok();
    }
}

#[cfg(target_os = "macos")]
pub(super) fn reload_page(app: &AppHandle, id: u32, hard: bool) {
    use objc2_web_kit::WKWebView;

    let Some(page) = page(app, id) else {
        return;
    };
    page.with_webview(move |platform| {
        let ptr = platform.inner() as *mut WKWebView;
        if ptr.is_null() {
            return;
        }
        let view = unsafe { &*ptr };
        unsafe {
            if hard {
                view.reloadFromOrigin();
            } else {
                view.reload();
            }
        }
    })
    .ok();
}

/// The script API has no cache-ignoring reload.
#[cfg(not(target_os = "macos"))]
pub(super) fn reload_page(app: &AppHandle, id: u32, _hard: bool) {
    if let Some(webview) = page(app, id) {
        webview.eval("location.reload()").ok();
    }
}

/// The same range as the app's own window zoom.
pub(super) const ZOOM_MIN: f64 = 0.5;
pub(super) const ZOOM_MAX: f64 = 3.0;

/// WebKit's `findString:withConfiguration:` highlights and scrolls itself;
/// its result says only whether anything matched — there is no match count.
#[cfg(target_os = "macos")]
pub(super) fn find_string(app: &AppHandle, id: u32, query: String, backwards: bool) {
    use block2::RcBlock;
    use objc2::MainThreadMarker;
    use objc2_foundation::NSString;
    use objc2_web_kit::{WKFindConfiguration, WKFindResult, WKWebView};

    let Some(page) = page(app, id) else {
        return;
    };
    let app = app.clone();
    page.with_webview(move |platform| {
        let ptr = platform.inner() as *mut WKWebView;
        let Some(mtm) = MainThreadMarker::new() else {
            return;
        };
        if ptr.is_null() {
            return;
        }
        let view = unsafe { &*ptr };
        let needle = NSString::from_str(&query);
        let config = unsafe { WKFindConfiguration::new(mtm) };
        unsafe {
            config.setBackwards(backwards);
            config.setWraps(true);
            config.setCaseSensitive(false);
        }
        let reply_app = app.clone();
        let echo = query.clone();
        // Never nil — see `seed_sessions`.
        let done = RcBlock::new(move |result: std::ptr::NonNull<WKFindResult>| {
            let found = unsafe { result.as_ref().matchFound() };
            reply_app
                .emit_to(
                    EventTarget::webview(MAIN),
                    "browser-find",
                    FindResult {
                        id,
                        query: echo.clone(),
                        found,
                    },
                )
                .ok();
        });
        unsafe {
            view.findString_withConfiguration_completionHandler(&needle, Some(&config), &done);
        }
    })
    .ok();
}

/// `window.find` reports nothing, so the bar is always told it matched.
#[cfg(not(target_os = "macos"))]
pub(super) fn find_string(app: &AppHandle, id: u32, query: String, backwards: bool) {
    if let Some(webview) = page(app, id) {
        let escaped = serde_json::to_string(&query).unwrap_or_else(|_| "\"\"".into());
        webview
            .eval(&format!("window.find({escaped}, false, {backwards}, true)"))
            .ok();
    }
    app.emit_to(
        EventTarget::webview(MAIN),
        "browser-find",
        FindResult {
            id,
            query,
            found: true,
        },
    )
    .ok();
}
