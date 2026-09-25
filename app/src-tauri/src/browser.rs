//! The in-app browser: one native WebView per tab, parked inside the main
//! window's content card, so Canvas/Ed/Echo360 links open signed in.
//!
//! - **Real WKWebViews, not iframes** — sites send `X-Frame-Options`.
//!   `Window::add_child` needs tauri's `unstable` feature.
//! - **Rust owns the tab list**; every change goes out whole as
//!   `browser-state`. The route for a tab is `/browse/<id>`, never the page URL.
//! - **The frontend reports each page's slot as insets** (`Viewport`), so a
//!   window resize is laid out here with no JavaScript in the loop. It asks for
//!   a page to be hidden when it must draw over it.
//! - Back/forward, find and snapshots go through the WKWebView directly;
//!   `with_webview` returns nothing, so answers are pushed as events.
//!
//! `canvas_session` is HttpOnly and session-scoped, so WebKit loses it on quit;
//! `seed_canvas_session` restores it from the scraper's snapshot, and each
//! Canvas page load re-snapshots it (see `docs/auth.md`).

use std::collections::{HashMap, HashSet};
use std::io::Read;
use std::sync::Mutex;
use std::time::Duration;

use base64::Engine as _;
use serde::{Deserialize, Serialize};
use tauri::webview::{NewWindowResponse, PageLoadEvent, Webview, WebviewBuilder};
use tauri::{
    AppHandle, Emitter, EventTarget, LogicalPosition, LogicalSize, Manager, WebviewUrl, Window,
    WindowEvent,
};

/// The window every page lives in, and the webview that hears about them.
pub const MAIN: &str = "main";
/// Page webviews are `browse-<tab id>`. Not in any capability: they hold
/// remote content and must reach no Tauri command.
pub const LABEL_PREFIX: &str = "browse-";

pub const CANVAS_HOST: &str = "canvas.lms.unimelb.edu.au";

/// WKWebView's default UA has no `Version/… Safari/…` suffix, and UA-sniffing
/// sites (google.com) serve their no-JavaScript fallback to it. Keep the
/// version roughly current. Deliberately separate from `okta.rs`'s string.
const PAGE_USER_AGENT: &str = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/26.3 Safari/605.1.15";

fn label(id: u32) -> String {
    format!("{LABEL_PREFIX}{id}")
}

#[derive(Clone, Serialize)]
pub struct Tab {
    pub id: u32,
    pub url: String,
    pub title: String,
    pub loading: bool,
    /// Read off the WKWebView's back/forward list after every page load.
    pub can_back: bool,
    pub can_forward: bool,
    /// The WKWebView's `pageZoom` (1.0 = 100%), per tab, never persisted.
    pub zoom: f64,
}

impl Tab {
    fn new(id: u32, url: String) -> Self {
        Self {
            id,
            url,
            title: String::new(),
            loading: true,
            can_back: false,
            can_forward: false,
            zoom: 1.0,
        }
    }
}

/// On its own event so kilobytes of icon don't ride every snapshot.
#[derive(Clone, Serialize)]
struct FaviconFound {
    host: String,
    /// A `data:` URL.
    icon: String,
}

#[derive(Clone, Serialize)]
struct FindResult {
    id: u32,
    query: String,
    found: bool,
}

/// Everything the frontend mirrors, sent whole on every change.
#[derive(Clone, Serialize)]
pub struct Snapshot {
    pub tabs: Vec<Tab>,
}

/// Insets from the window's content edges in logical points, plus the
/// bottom-corner radius of the card the page sits in.
#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
pub struct Viewport {
    pub left: f64,
    pub top: f64,
    pub right: f64,
    pub bottom: f64,
    pub radius: f64,
}

#[derive(Default)]
struct Inner {
    tabs: Vec<Tab>,
    next_id: u32,
    /// Absent until the page is first placed; a page is hidden until then.
    viewports: HashMap<u32, Viewport>,
    /// One favicon attempt per host per run, found or not; the frontend
    /// persists what was found.
    favicons_tried: HashSet<String>,
}

#[derive(Default)]
pub struct BrowserState(Mutex<Inner>);

fn with_state<T>(app: &AppHandle, f: impl FnOnce(&mut Inner) -> T) -> T {
    let state = app.state::<BrowserState>();
    let mut inner = state.0.lock().unwrap_or_else(|e| e.into_inner());
    f(&mut inner)
}

fn snapshot(app: &AppHandle) -> Snapshot {
    with_state(app, |s| Snapshot {
        tabs: s.tabs.clone(),
    })
}

/// Targeted at the app's webview only — pages must not hear about each other.
fn broadcast(app: &AppHandle) {
    app.emit_to(EventTarget::webview(MAIN), "browser-state", snapshot(app))
        .ok();
}

// ── Cookie seeding ──────────────────────────────────────────────────────

/// Copies the persisted Canvas session header (bare `name=value; …` pairs)
/// into WebKit's cookie jar, scoped to the Canvas host. Safe to redo before
/// each page: WebKit replaces same-name cookies.
#[cfg(target_os = "macos")]
pub fn seed_canvas_session(app: &AppHandle) {
    use block2::RcBlock;
    use objc2::runtime::AnyObject;
    use objc2::MainThreadMarker;
    use objc2_foundation::{
        NSArray, NSDictionary, NSHTTPCookie, NSHTTPCookieDomain, NSHTTPCookieName,
        NSHTTPCookiePath, NSHTTPCookieSecure, NSHTTPCookieValue, NSString,
    };
    use objc2_web_kit::WKWebsiteDataStore;

    let header = crate::auth::saved_cookie_header();
    if header.trim().is_empty() {
        return;
    }

    // `defaultDataStore` is main-thread-only, and so is everything downstream.
    app.run_on_main_thread(move || {
        let Some(mtm) = MainThreadMarker::new() else {
            return;
        };
        let domain = NSString::from_str(CANVAS_HOST);
        let path = NSString::from_str("/");
        let secure = NSString::from_str("TRUE");

        let cookies: Vec<_> = header
            .split(';')
            .filter_map(|pair| pair.trim().split_once('='))
            .filter_map(|(name, value)| {
                let name = NSString::from_str(name.trim());
                let value = NSString::from_str(value.trim());
                let keys: [&NSString; 5] = unsafe {
                    [
                        NSHTTPCookieName,
                        NSHTTPCookieValue,
                        NSHTTPCookieDomain,
                        NSHTTPCookiePath,
                        NSHTTPCookieSecure,
                    ]
                };
                let values: [&AnyObject; 5] = [&name, &value, &domain, &path, &secure];
                let props = NSDictionary::from_slices(&keys, &values);
                unsafe { NSHTTPCookie::cookieWithProperties(&props) }
            })
            .collect();
        if cookies.is_empty() {
            return;
        }
        let count = cookies.len();
        let array = NSArray::from_retained_slice(&cookies);
        // Never pass a nil completion handler: WebKit invokes it anyway and
        // the app segfaults later, far from here.
        let done = RcBlock::new(|| {});
        unsafe {
            WKWebsiteDataStore::defaultDataStore(mtm)
                .httpCookieStore()
                .setCookies_completionHandler(&array, Some(&done));
        }
        eprintln!("[oculus] browser: seeded {count} Canvas cookies into WebKit");
    })
    .ok();
}

#[cfg(not(target_os = "macos"))]
pub fn seed_canvas_session(_app: &AppHandle) {}

// ── Layout ──────────────────────────────────────────────────────────────

/// Seeds the cookie jar and hooks the main window's resize. From `setup`.
pub fn init(app: &AppHandle) {
    seed_canvas_session(app);
    let Some(window) = app.get_window(MAIN) else {
        eprintln!("[oculus] browser: no main window at setup; pages will not follow resizes");
        return;
    };
    let events_app = app.clone();
    window.on_window_event(move |event| {
        if matches!(
            event,
            WindowEvent::Resized(_) | WindowEvent::ScaleFactorChanged { .. }
        ) {
            layout(&events_app);
        }
    });
}

fn parse(url: &str) -> Result<url::Url, String> {
    let parsed: url::Url = url.parse().map_err(|e| format!("bad url {url}: {e}"))?;
    match parsed.scheme() {
        // `file://` would expose the disk; custom schemes are the app's own.
        "http" | "https" => Ok(parsed),
        other => Err(format!("refusing to open {other}: scheme")),
    }
}

fn page(app: &AppHandle, id: u32) -> Option<Webview<tauri::Wry>> {
    app.get_webview(&label(id))
}

fn tab_id(label: &str) -> Option<u32> {
    label.strip_prefix(LABEL_PREFIX)?.parse().ok()
}

fn pages(app: &AppHandle) -> Vec<Webview<tauri::Wry>> {
    app.webviews()
        .into_iter()
        .filter(|(label, _)| label.starts_with(LABEL_PREFIX))
        .map(|(_, webview)| webview)
        .collect()
}

/// The window's content area in logical points.
fn content_size(window: &Window) -> LogicalSize<f64> {
    let scale = window.scale_factor().unwrap_or(1.0);
    window
        .inner_size()
        .map(|s| s.to_logical::<f64>(scale))
        .unwrap_or_else(|_| LogicalSize::new(1480.0, 920.0))
}

/// One page's rect: the content area minus its insets. An unplaced (hidden)
/// page takes the whole window.
fn rect(
    window: LogicalSize<f64>,
    viewport: Option<Viewport>,
) -> (LogicalPosition<f64>, LogicalSize<f64>, f64) {
    let Some(vp) = viewport else {
        return (LogicalPosition::new(0.0, 0.0), window, 0.0);
    };
    let width = (window.width - vp.left - vp.right).max(1.0);
    let height = (window.height - vp.top - vp.bottom).max(1.0);
    (
        LogicalPosition::new(vp.left, vp.top),
        LogicalSize::new(width, height),
        vp.radius.max(0.0),
    )
}

/// Rounds a page's bottom corners to the card's radius; a native view over
/// the DOM has to clip itself.
#[cfg(target_os = "macos")]
fn round_corners(page: &Webview<tauri::Wry>, radius: f64) {
    use objc2::msg_send;
    use objc2::runtime::AnyObject;

    page.with_webview(move |platform| unsafe {
        let view = platform.inner() as *mut AnyObject;
        if view.is_null() {
            return;
        }
        let _: () = msg_send![view, setWantsLayer: true];
        let layer: *mut AnyObject = msg_send![view, layer];
        if layer.is_null() {
            return;
        }
        // CACornerMask: MinXMaxY | MaxXMaxY. WKWebView is a flipped view, so
        // its layer's MaxY edge is the bottom.
        let mask: usize = (1 << 2) | (1 << 3);
        let _: () = msg_send![layer, setMasksToBounds: true];
        let _: () = msg_send![layer, setCornerRadius: radius];
        let _: () = msg_send![layer, setMaskedCorners: mask];
    })
    .ok();
}

#[cfg(not(target_os = "macos"))]
fn round_corners(_page: &Webview<tauri::Wry>, _radius: f64) {}

/// The resize handler: puts every placed page back in its slot. Unplaced
/// pages are hidden and skipped.
fn layout(app: &AppHandle) {
    let Some(window) = app.get_window(MAIN) else {
        return;
    };
    let bounds = content_size(&window);
    let viewports = with_state(app, |s| s.viewports.clone());
    for page in pages(app) {
        let Some(vp) = tab_id(page.label()).and_then(|id| viewports.get(&id).copied()) else {
            continue;
        };
        let (position, size, radius) = rect(bounds, Some(vp));
        page.set_position(position).ok();
        page.set_size(size).ok();
        round_corners(&page, radius);
    }
}

fn layout_tab(app: &AppHandle, id: u32) {
    let (Some(window), Some(page)) = (app.get_window(MAIN), page(app, id)) else {
        return;
    };
    let viewport = with_state(app, |s| s.viewports.get(&id).copied());
    let (position, size, radius) = rect(content_size(&window), viewport);
    page.set_position(position).ok();
    page.set_size(size).ok();
    round_corners(&page, radius);
}

/// Attaches a page webview to a tab. It starts hidden until the frontend
/// places it.
fn create_page(app: &AppHandle, id: u32, url: url::Url) -> Result<(), String> {
    let window = app.get_window(MAIN).ok_or("no main window")?;
    if url.host_str() == Some(CANVAS_HOST) {
        seed_canvas_session(app);
    }

    let load_app = app.clone();
    let title_app = app.clone();
    let popup_app = app.clone();
    let builder = WebviewBuilder::new(label(id), WebviewUrl::External(url))
        .user_agent(PAGE_USER_AGENT)
        // A child webview reports `outerWidth`/`outerHeight` as 0, and canvas
        // renderers (Google Docs) read `outerWidth / innerWidth` as zoom and
        // drop to minimum resolution. Report the viewport's size instead.
        .initialization_script(
            r#"(function(){try{var d=function(k,s){Object.defineProperty(window,k,{configurable:true,get:function(){return window[s];}});};d('outerWidth','innerWidth');d('outerHeight','innerHeight');}catch(e){}})()"#,
        )
        // Page-load events (main frame only), not `on_navigation`, which
        // fires for every iframe — Canvas's LTI shims would hijack the URL.
        .on_page_load(move |webview, payload| {
            let started = matches!(payload.event(), PageLoadEvent::Started);
            let url = payload.url().to_string();
            with_state(&load_app, |s| {
                if let Some(tab) = s.tabs.iter_mut().find(|t| t.id == id) {
                    tab.url = url;
                    tab.loading = started;
                }
            });
            broadcast(&load_app);
            // Both edges: asking twice covers a page that redirects on arrival.
            refresh_nav(&load_app, id);
            if !started {
                // A Canvas load rolls the session forward; re-snapshot it.
                if payload.url().host_str() == Some(CANVAS_HOST) {
                    crate::auth::save_session_cookie(webview.app_handle());
                }
                ensure_favicon(&load_app, payload.url());
            }
        })
        .on_document_title_changed(move |_, title| {
            with_state(&title_app, |s| {
                if let Some(tab) = s.tabs.iter_mut().find(|t| t.id == id) {
                    tab.title = title;
                }
            });
            broadcast(&title_app);
        })
        // `target=_blank`/`window.open` become a new tab. On the main thread
        // `run_on_main_thread` runs inline, inside WebKit's delegate callback,
        // so hop through a helper thread to queue it behind the callback.
        .on_new_window(move |url, _| {
            let app = popup_app.clone();
            std::thread::spawn(move || {
                app.clone()
                    .run_on_main_thread(move || {
                        open_tab(&app, url).ok();
                    })
                    .ok();
            });
            NewWindowResponse::Deny
        });

    let viewport = with_state(app, |s| s.viewports.get(&id).copied());
    let (position, size, radius) = rect(content_size(&window), viewport);
    let webview = window
        .add_child(builder, position, size)
        .map_err(|e| format!("failed to open page webview: {e}"))?;
    webview.hide().ok();
    round_corners(&webview, radius);
    Ok(())
}

fn hide_all(app: &AppHandle) {
    for page in pages(app) {
        page.hide().ok();
    }
}

/// Opens `url` in a new tab — the entry point for every link in the app.
pub fn open_tab(app: &AppHandle, url: url::Url) -> Result<u32, String> {
    let id = with_state(app, |s| {
        s.next_id += 1;
        s.tabs.push(Tab::new(s.next_id, url.to_string()));
        s.next_id
    });
    eprintln!("[oculus] browser: opening tab {id} → {url}");
    if let Err(e) = create_page(app, id, url) {
        with_state(app, |s| s.tabs.retain(|t| t.id != id));
        return Err(e);
    }
    broadcast(app);
    Ok(id)
}

// ── The page's own state ────────────────────────────────────────────────
//
// Tauri has no API for history state or find, so these go through the
// WKWebView. `with_webview` returns nothing, so each answer is written into
// the tab (or an event) and broadcast from inside the closure.

#[cfg(target_os = "macos")]
fn refresh_nav(app: &AppHandle, id: u32) {
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
fn refresh_nav(app: &AppHandle, id: u32) {
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
fn go_history(app: &AppHandle, id: u32, delta: i32) {
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
fn go_history(app: &AppHandle, id: u32, delta: i32) {
    if let Some(webview) = page(app, id) {
        webview.eval(&format!("history.go({delta})")).ok();
    }
}

#[cfg(target_os = "macos")]
fn reload_page(app: &AppHandle, id: u32, hard: bool) {
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
fn reload_page(app: &AppHandle, id: u32, _hard: bool) {
    if let Some(webview) = page(app, id) {
        webview.eval("location.reload()").ok();
    }
}

/// The same range as the app's own window zoom.
const ZOOM_MIN: f64 = 0.5;
const ZOOM_MAX: f64 = 3.0;

/// WebKit's `findString:withConfiguration:` highlights and scrolls itself;
/// its result says only whether anything matched — there is no match count.
#[cfg(target_os = "macos")]
fn find_string(app: &AppHandle, id: u32, query: String, backwards: bool) {
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
        // Never nil — see `seed_canvas_session`.
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
fn find_string(app: &AppHandle, id: u32, query: String, backwards: bool) {
    if let Some(webview) = page(app, id) {
        let escaped = serde_json::to_string(&query).unwrap_or_else(|_| "\"\"".into());
        webview
            .eval(&format!(
                "window.find({escaped}, false, {backwards}, true)"
            ))
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

// ── A still of the page ─────────────────────────────────────────────────
//
// The DOM cannot draw over a native view, so for a popup the frontend paints
// a snapshot of the page in its slot and hides the live page behind it.
// Raw `msg_send!`: the typed wrapper returns `NSImage` and would pull in all
// of AppKit.

/// The block is `Fn` but the send consumes the sender, hence the lock.
#[cfg(target_os = "macos")]
fn deliver(
    cell: &Mutex<Option<tokio::sync::oneshot::Sender<Option<Vec<u8>>>>>,
    png: Option<Vec<u8>>,
) {
    if let Some(tx) = cell.lock().ok().and_then(|mut slot| slot.take()) {
        let _ = tx.send(png);
    }
}

/// `NSImage` → TIFF → PNG, at the image's own pixel size (2x stays 2x).
#[cfg(target_os = "macos")]
unsafe fn png_bytes(image: *mut objc2::runtime::AnyObject) -> Option<Vec<u8>> {
    use objc2::runtime::AnyObject;
    use objc2::{class, msg_send};

    if image.is_null() {
        return None;
    }
    let tiff: *mut AnyObject = msg_send![image, TIFFRepresentation];
    if tiff.is_null() {
        return None;
    }
    let rep: *mut AnyObject = msg_send![class!(NSBitmapImageRep), imageRepWithData: tiff];
    if rep.is_null() {
        return None;
    }
    let props: *mut AnyObject = msg_send![class!(NSDictionary), dictionary];
    // NSBitmapImageFileTypePNG.
    let data: *mut AnyObject = msg_send![rep, representationUsingType: 4usize, properties: props];
    if data.is_null() {
        return None;
    }
    let len: usize = msg_send![data, length];
    let bytes: *const u8 = msg_send![data, bytes];
    if bytes.is_null() || len == 0 {
        return None;
    }
    Some(std::slice::from_raw_parts(bytes, len).to_vec())
}

#[cfg(target_os = "macos")]
fn snapshot_page(
    app: &AppHandle,
    id: u32,
    reply: tokio::sync::oneshot::Sender<Option<Vec<u8>>>,
) {
    use block2::RcBlock;
    use objc2::msg_send;
    use objc2::runtime::AnyObject;

    let cell = std::sync::Arc::new(Mutex::new(Some(reply)));
    let Some(page) = page(app, id) else {
        deliver(&cell, None);
        return;
    };
    let outer = cell.clone();
    let queued = page.with_webview(move |platform| {
        let view = platform.inner() as *mut AnyObject;
        if view.is_null() {
            deliver(&outer, None);
            return;
        }
        let inner = outer.clone();
        let done = RcBlock::new(move |image: *mut AnyObject, _error: *mut AnyObject| {
            deliver(&inner, unsafe { png_bytes(image) });
        });
        // A nil configuration snapshots the visible viewport.
        unsafe {
            let config: *mut AnyObject = std::ptr::null_mut();
            let _: () = msg_send![
                view,
                takeSnapshotWithConfiguration: config,
                completionHandler: &*done,
            ];
        }
    });
    // Nothing will call the block, so release the waiter here.
    if queued.is_err() {
        deliver(&cell, None);
    }
}

#[cfg(not(target_os = "macos"))]
fn snapshot_page(
    _app: &AppHandle,
    _id: u32,
    reply: tokio::sync::oneshot::Sender<Option<Vec<u8>>>,
) {
    let _ = reply.send(None);
}

// ── Favicons ────────────────────────────────────────────────────────────
//
// WebKit has no public favicon API, so icons are fetched over HTTP by host:
// `/favicon.ico` first, then the document's `<link rel=icon>`.

const FAVICON_MAX: usize = 256 * 1024;
/// Enough of a document to hold its `<head>`.
const FAVICON_HTML_MAX: u64 = 512 * 1024;

fn favicon_agent() -> ureq::Agent {
    ureq::AgentBuilder::new()
        .timeout(Duration::from_secs(8))
        .user_agent(PAGE_USER_AGENT)
        .build()
}

fn ensure_favicon(app: &AppHandle, url: &url::Url) {
    let Some(host) = url.host_str().map(str::to_owned) else {
        return;
    };
    let first = with_state(app, |s| s.favicons_tried.insert(host.clone()));
    if !first {
        return;
    }
    let origin = url.origin().ascii_serialization();
    let page_url = url.to_string();
    let app = app.clone();
    std::thread::spawn(move || {
        let Some((mime, bytes)) = favicon_for(&origin, &page_url) else {
            return;
        };
        let icon = format!(
            "data:{mime};base64,{}",
            base64::engine::general_purpose::STANDARD.encode(&bytes)
        );
        app.emit_to(
            EventTarget::webview(MAIN),
            "browser-favicon",
            FaviconFound { host, icon },
        )
        .ok();
    });
}

fn favicon_for(origin: &str, page_url: &str) -> Option<(String, Vec<u8>)> {
    if let Some(found) = fetch_icon(&format!("{origin}/favicon.ico")) {
        return Some(found);
    }
    let html = fetch_head(page_url)?;
    let href = declared_icon(&html)?;
    let resolved = url::Url::parse(page_url).ok()?.join(&href).ok()?;
    if !matches!(resolved.scheme(), "http" | "https") {
        return None;
    }
    fetch_icon(resolved.as_str())
}

fn fetch_icon(url: &str) -> Option<(String, Vec<u8>)> {
    let response = favicon_agent().get(url).call().ok()?;
    let content_type = response.header("content-type").unwrap_or("").to_lowercase();
    let mut bytes = Vec::new();
    response
        .into_reader()
        .take(FAVICON_MAX as u64 + 1)
        .read_to_end(&mut bytes)
        .ok()?;
    if bytes.is_empty() || bytes.len() > FAVICON_MAX {
        return None;
    }
    let mime = sniff_image(&content_type, &bytes)?;
    Some((mime, bytes))
}

fn fetch_head(url: &str) -> Option<String> {
    let response = favicon_agent().get(url).call().ok()?;
    let mut body = Vec::new();
    response
        .into_reader()
        .take(FAVICON_HTML_MAX)
        .read_to_end(&mut body)
        .ok()?;
    Some(String::from_utf8_lossy(&body).into_owned())
}

/// Magic numbers before the header: many servers answer `/favicon.ico` with
/// an HTML 404 page under a 200.
fn sniff_image(content_type: &str, bytes: &[u8]) -> Option<String> {
    if bytes.starts_with(b"\x89PNG") {
        return Some("image/png".into());
    }
    if bytes.starts_with(b"GIF8") {
        return Some("image/gif".into());
    }
    if bytes.starts_with(&[0xFF, 0xD8, 0xFF]) {
        return Some("image/jpeg".into());
    }
    if bytes.starts_with(b"RIFF") && bytes.get(8..12) == Some(b"WEBP") {
        return Some("image/webp".into());
    }
    if bytes.starts_with(&[0x00, 0x00, 0x01, 0x00]) {
        return Some("image/x-icon".into());
    }
    let head = String::from_utf8_lossy(&bytes[..bytes.len().min(512)]).to_lowercase();
    if head.contains("<svg") {
        return Some("image/svg+xml".into());
    }
    if head.contains("<html") || head.contains("<!doctype html") {
        return None;
    }
    content_type
        .starts_with("image/")
        .then(|| content_type.split(';').next().unwrap_or_default().trim().to_string())
        .filter(|mime| !mime.is_empty())
}

/// The largest icon a document declares. `rel~="icon"` is a whole-word match,
/// so `apple-touch-icon` is skipped.
fn declared_icon(html: &str) -> Option<String> {
    let document = scraper::Html::parse_document(html);
    let selector = scraper::Selector::parse(r#"link[rel~="icon"]"#).ok()?;
    let mut best: Option<(u32, String)> = None;
    for link in document.select(&selector) {
        let Some(href) = link.value().attr("href").map(str::trim) else {
            continue;
        };
        if href.is_empty() {
            continue;
        }
        // "any" is what an SVG declares.
        let size = match link.value().attr("sizes").map(str::to_lowercase) {
            Some(s) if s.contains("any") => u32::MAX,
            Some(s) => s
                .split_whitespace()
                .filter_map(|pair| pair.split(['x', 'X']).next()?.parse::<u32>().ok())
                .max()
                .unwrap_or(0),
            None => 0,
        };
        if best.as_ref().is_none_or(|(seen, _)| size > *seen) {
            best = Some((size, href.to_string()));
        }
    }
    best.map(|(_, href)| href)
}

// ── Commands ────────────────────────────────────────────────────────────

#[tauri::command]
pub fn browser_open_url(app: AppHandle, url: String) -> Result<u32, String> {
    open_tab(&app, parse(&url)?)
}

/// For the frontend's mount, which may have missed earlier events.
#[tauri::command]
pub fn browser_state(app: AppHandle) -> Snapshot {
    snapshot(&app)
}

/// A slot for tab `id` has mounted: place and show that page only.
#[tauri::command]
pub fn browser_place(app: AppHandle, id: u32, viewport: Viewport) {
    with_state(&app, |s| {
        s.viewports.insert(id, viewport);
    });
    layout_tab(&app, id);
    if let Some(page) = page(&app, id) {
        page.show().ok();
        page.set_focus().ok();
    }
}

/// Placement only — a hidden page stays hidden.
#[tauri::command]
pub fn browser_set_viewport(app: AppHandle, id: u32, viewport: Viewport) {
    with_state(&app, |s| {
        s.viewports.insert(id, viewport);
    });
    layout_tab(&app, id);
}

/// Hidden, not destroyed.
#[tauri::command]
pub fn browser_hide_tab(app: AppHandle, id: u32) {
    if let Some(page) = page(&app, id) {
        page.hide().ok();
    }
}

/// PNG bytes (raw, not base64 — a 2x still is megabytes). On error the
/// caller hides the page instead.
#[tauri::command]
pub async fn browser_snapshot(app: AppHandle, id: u32) -> Result<tauri::ipc::Response, String> {
    let (tx, rx) = tokio::sync::oneshot::channel();
    snapshot_page(&app, id, tx);
    match rx.await {
        Ok(Some(png)) => Ok(tauri::ipc::Response::new(png)),
        _ => Err("no snapshot".into()),
    }
}

#[tauri::command]
pub fn browser_hide(app: AppHandle) {
    hide_all(&app);
}

/// Always navigates, even to the same URL.
#[tauri::command]
pub fn browser_navigate(app: AppHandle, id: u32, url: String) -> Result<(), String> {
    let target = parse(&url)?;
    let webview = page(&app, id).ok_or("no such tab")?;
    if target.host_str() == Some(CANVAS_HOST) {
        seed_canvas_session(&app);
    }
    webview.navigate(target).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn browser_history(app: AppHandle, id: u32, delta: i32) {
    go_history(&app, id, delta);
}

/// `hard` uses WKWebView's `reloadFromOrigin`, which revalidates; a plain
/// reload can keep serving a cached response.
#[tauri::command]
pub fn browser_reload(app: AppHandle, id: u32, hard: bool) {
    reload_page(&app, id, hard);
}

/// The page's own zoom (`pageZoom`), not the app's window zoom.
#[tauri::command]
pub fn browser_set_zoom(app: AppHandle, id: u32, zoom: f64) {
    let zoom = if zoom.is_finite() {
        zoom.clamp(ZOOM_MIN, ZOOM_MAX)
    } else {
        1.0
    };
    let Some(webview) = page(&app, id) else {
        return;
    };
    webview.set_zoom(zoom).ok();
    with_state(&app, |s| {
        if let Some(tab) = s.tabs.iter_mut().find(|t| t.id == id) {
            tab.zoom = zoom;
        }
    });
    broadcast(&app);
}

/// The answer comes back as a `browser-find` event.
#[tauri::command]
pub fn browser_find(app: AppHandle, id: u32, query: String, backwards: bool) {
    if query.is_empty() {
        browser_find_clear(app, id);
        return;
    }
    find_string(&app, id, query, backwards);
}

/// WebKit's find leaves an ordinary selection, so clearing it is the whole job.
#[tauri::command]
pub fn browser_find_clear(app: AppHandle, id: u32) {
    if let Some(webview) = page(&app, id) {
        webview
            .eval("try { window.getSelection().removeAllRanges(); } catch {}")
            .ok();
    }
}

/// **`close()` alone does not stop the page.** wry's macOS `Drop` leaks the
/// WKWebView (`removeFromSuperview` + `retain`), which keeps playing media
/// unreachably. So tear the document down while this handle still exists.
#[tauri::command]
pub fn browser_close_tab(app: AppHandle, id: u32) {
    if let Some(webview) = page(&app, id) {
        // Pause lands immediately; `about:blank` commits later, on the
        // leaked view, and destroys the media elements.
        webview
            .eval(
                "for (const m of document.querySelectorAll('video,audio')) \
                 { try { m.pause(); m.removeAttribute('src'); m.load(); } catch {} }",
            )
            .ok();
        if let Ok(blank) = "about:blank".parse() {
            webview.navigate(blank).ok();
        }
        webview.close().ok();
    }
    with_state(&app, |s| {
        s.tabs.retain(|t| t.id != id);
        s.viewports.remove(&id);
    });
    broadcast(&app);
}
