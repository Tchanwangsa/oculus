//! The in-app browser: one native WebView per tab, parked inside the main
//! window's content card.
//!
//! Any external link in the app opens here instead of the system browser, so
//! Canvas, Ed and Echo360 pages stay inside Oculus and — the part that costs
//! something — stay signed in. Three decisions shape this module:
//!
//! - **Pages are real WKWebViews, not iframes.** Canvas (and every other
//!   site worth opening) sends `X-Frame-Options`/`frame-ancestors`, which
//!   WebKit enforces — an iframe would render a blank box. `Window::add_child`
//!   makes each page a sibling of the app's own webview instead, stacked
//!   above it; that needs tauri's `unstable` feature (multiwebview).
//! - **Rust owns the tab list.** The page's own events (a redirect, a title
//!   change, a `target=_blank` link) land here first, so this is the only
//!   place that can be right about what each tab holds. Every change is
//!   pushed whole to the main webview as `browser-state`; the frontend
//!   mirrors the list into its tab strip and holds nothing but the address
//!   bar's draft. Page URLs never touch the React router — the route for a
//!   browser tab is `/browse/<id>`, stable for the life of the tab — which is
//!   what keeps a page load from re-laying-out the page that fired it.
//! - **The frontend says where the page goes, Rust puts it there.** A native
//!   view cannot interleave with the DOM, so the page lives in a slot the
//!   React page leaves for it and the frontend reports that slot as insets
//!   from the window's edges (`Viewport`). Insets, not a rect: a window
//!   resize is then laid out here from the window size alone, with no
//!   JavaScript in the loop to lag behind it. The frontend only speaks up
//!   when the insets themselves change — sidebar, zoom — and when something
//!   of its own has to draw over the page, which is the one thing a native
//!   view cannot allow: it asks for the page to be hidden until the popup
//!   is gone.
//!
//! Signed in, not for free: `canvas_session` is HttpOnly *and* session-scoped,
//! so WebKit holds it in memory only and it is gone when the app quits — the
//! scraper's snapshot on disk is the only copy that survives.
//! `seed_canvas_session` puts it back into WebKit's jar before a Canvas page
//! loads. (`/login/session_token`, the API built for exactly this, answers
//! 403 here: it wants an access token, and UniMelb has those disabled — see
//! `docs/auth.md`.) Browsing Canvas then rolls the session forward, so every
//! Canvas page load re-snapshots the cookie and the scraper inherits it.

use std::sync::Mutex;

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

fn label(id: u32) -> String {
    format!("{LABEL_PREFIX}{id}")
}

#[derive(Clone, Serialize)]
pub struct Tab {
    pub id: u32,
    pub url: String,
    pub title: String,
    pub loading: bool,
}

/// Everything the frontend mirrors, sent whole on every change. Diffing
/// would mean two copies of the truth.
#[derive(Clone, Serialize)]
pub struct Snapshot {
    pub tabs: Vec<Tab>,
}

/// Where pages go, as insets from the window's content edges in logical
/// points, plus the corner radius the bottom corners take so the page fits
/// the rounded card it sits in.
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
    /// Last reported by the frontend; `None` until a browser tab has been
    /// shown once. Pages created before then are hidden anyway.
    viewport: Option<Viewport>,
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

/// Tells the frontend what the tabs hold. Targeted at the app's webview:
/// the pages themselves have no business hearing about each other.
fn broadcast(app: &AppHandle) {
    app.emit_to(EventTarget::webview(MAIN), "browser-state", snapshot(app))
        .ok();
}

// ── Cookie seeding ──────────────────────────────────────────────────────

/// Copies the persisted Canvas session into WebKit's shared cookie jar, so a
/// Canvas page opened in the browser is signed in as the scraper is.
///
/// The snapshot is a bare `name=value; …` header — whatever the login window
/// held, with no domains — and it is replayed to Canvas verbatim on every
/// scrape, so the faithful reconstruction is exactly that: every pair, scoped
/// to the Canvas host, expiring with the session. Cheap enough to redo before
/// each page: WebKit replaces same-name cookies rather than duplicating them.
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

    let header = crate::auth::saved_cookie_header(app);
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
        // The completion handler is not optional in practice: WebKit invokes
        // whatever it was handed when the cookie process replies, and passing
        // nil segfaults the app a second later, far from here.
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

/// Seeds the cookie jar and hooks the main window's resize, so pages follow
/// the window without a round trip through JavaScript. From `setup`, once
/// the config windows exist.
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
        // Only ever hand a page webview web content: `file://` would expose
        // the disk, and custom schemes are the app's own.
        "http" | "https" => Ok(parsed),
        other => Err(format!("refusing to open {other}: scheme")),
    }
}

fn page(app: &AppHandle, id: u32) -> Option<Webview<tauri::Wry>> {
    app.get_webview(&label(id))
}

/// Every page webview, whatever tab it belongs to.
fn pages(app: &AppHandle) -> Vec<Webview<tauri::Wry>> {
    app.webviews()
        .into_iter()
        .filter(|(label, _)| label.starts_with(LABEL_PREFIX))
        .map(|(_, webview)| webview)
        .collect()
}

/// The page rect in logical points: the window's content area minus the
/// reported insets. Before any viewport was reported, the whole window —
/// nothing is visible then, so it only has to be somewhere.
fn rect(app: &AppHandle, window: &Window) -> (LogicalPosition<f64>, LogicalSize<f64>, f64) {
    let scale = window.scale_factor().unwrap_or(1.0);
    let size = window
        .inner_size()
        .map(|s| s.to_logical::<f64>(scale))
        .unwrap_or_else(|_| LogicalSize::new(1480.0, 920.0));
    let Some(vp) = with_state(app, |s| s.viewport) else {
        return (LogicalPosition::new(0.0, 0.0), size, 0.0);
    };
    let width = (size.width - vp.left - vp.right).max(1.0);
    let height = (size.height - vp.top - vp.bottom).max(1.0);
    (
        LogicalPosition::new(vp.left, vp.top),
        LogicalSize::new(width, height),
        vp.radius.max(0.0),
    )
}

/// Rounds a page's bottom corners to the card's radius. The card is the
/// app's DOM and the page is a native view over it, so the page has to clip
/// itself; its top edge runs under the toolbar and stays square.
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

/// Puts every page in the slot. Called on each window resize and whenever
/// the frontend reports new insets.
fn layout(app: &AppHandle) {
    let Some(window) = app.get_window(MAIN) else {
        return;
    };
    let (position, size, radius) = rect(app, &window);
    for page in pages(app) {
        page.set_position(position).ok();
        page.set_size(size).ok();
        round_corners(&page, radius);
    }
}

/// Attaches a page webview to a tab, pointed at `url`. It starts hidden: the
/// frontend shows it once its route has mounted and reported where it goes,
/// so a page never appears somewhere the app is still drawing.
fn create_page(app: &AppHandle, id: u32, url: url::Url) -> Result<(), String> {
    let window = app.get_window(MAIN).ok_or("no main window")?;
    if url.host_str() == Some(CANVAS_HOST) {
        seed_canvas_session(app);
    }

    let load_app = app.clone();
    let title_app = app.clone();
    let popup_app = app.clone();
    let builder = WebviewBuilder::new(label(id), WebviewUrl::External(url))
        // Page-load events, not `on_navigation`, are what the tab follows:
        // `on_navigation` fires for every frame, and a Canvas dashboard is a
        // nest of iframes — the tab would end up pointed at an LTI
        // postMessage shim seconds after landing on the page you asked for.
        // These come from WebKit's navigation delegate, main frame only.
        .on_page_load(move |webview, payload| {
            let started = matches!(payload.event(), PageLoadEvent::Started);
            let url = payload.url().to_string();
            with_state(&load_app, |s| {
                if let Some(tab) = s.tabs.iter_mut().find(|t| t.id == id) {
                    // Commit, not finish: the address bar should say where
                    // you are going while it loads, as a browser does.
                    tab.url = url;
                    tab.loading = started;
                }
            });
            broadcast(&load_app);
            // A Canvas page load rolls the session forward; the cookie the
            // scraper replays is a snapshot, so take a fresh one.
            if !started && payload.url().host_str() == Some(CANVAS_HOST) {
                crate::auth::save_session_cookie(webview.app_handle());
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
        // `target=_blank` and `window.open` become a new tab. This runs
        // inside WebKit's delegate callback on the main thread, where
        // `run_on_main_thread` executes *inline* — so the tab is opened from
        // a helper thread, which queues it behind the callback instead.
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

    let (position, size, radius) = rect(app, &window);
    let webview = window
        .add_child(builder, position, size)
        .map_err(|e| format!("failed to open page webview: {e}"))?;
    // Commands run on the main thread, so nothing paints between the view
    // appearing and this: it is hidden before its first frame.
    webview.hide().ok();
    round_corners(&webview, radius);
    Ok(())
}

/// Shows one tab's page and hides the rest. Hidden, not destroyed: a tab you
/// come back to is still on the page you left it on.
fn show_only(app: &AppHandle, id: u32) {
    let target = label(id);
    for page in pages(app) {
        if page.label() == target {
            page.show().ok();
            page.set_focus().ok();
        } else {
            page.hide().ok();
        }
    }
}

fn hide_all(app: &AppHandle) {
    for page in pages(app) {
        page.hide().ok();
    }
}

/// Opens `url` in a new tab. The entry point for every link in the app; the
/// frontend hears about the tab through `browser-state` and brings it to
/// the front.
pub fn open_tab(app: &AppHandle, url: url::Url) -> Result<u32, String> {
    let id = with_state(app, |s| {
        s.next_id += 1;
        s.tabs.push(Tab {
            id: s.next_id,
            url: url.to_string(),
            title: String::new(),
            loading: true,
        });
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

// ── Commands ────────────────────────────────────────────────────────────

/// Open an external link. Returns the new tab's id.
#[tauri::command]
pub fn browser_open_url(app: AppHandle, url: String) -> Result<u32, String> {
    open_tab(&app, parse(&url)?)
}

/// What the frontend asks for on mount — it may have missed every event
/// before it loaded (a dev reload, say).
#[tauri::command]
pub fn browser_state(app: AppHandle) -> Snapshot {
    snapshot(&app)
}

/// The browser route has mounted for tab `id`: this is where the page goes,
/// show it there.
#[tauri::command]
pub fn browser_show(app: AppHandle, id: u32, viewport: Viewport) {
    with_state(&app, |s| s.viewport = Some(viewport));
    layout(&app);
    if with_state(&app, |s| s.tabs.iter().any(|t| t.id == id)) {
        show_only(&app, id);
    }
}

/// The slot moved or resized in a way the window size does not explain: the
/// sidebar toggled, the zoom changed.
#[tauri::command]
pub fn browser_set_viewport(app: AppHandle, viewport: Viewport) {
    with_state(&app, |s| s.viewport = Some(viewport));
    layout(&app);
}

/// The browser route unmounted, or the app needs to draw over the page.
#[tauri::command]
pub fn browser_hide(app: AppHandle) {
    hide_all(&app);
}

/// The address bar. Always navigates, even to the same URL — typing an
/// address and hitting return should reload it.
#[tauri::command]
pub fn browser_navigate(app: AppHandle, id: u32, url: String) -> Result<(), String> {
    let target = parse(&url)?;
    let webview = page(&app, id).ok_or("no such tab")?;
    if target.host_str() == Some(CANVAS_HOST) {
        seed_canvas_session(&app);
    }
    webview.navigate(target).map_err(|e| e.to_string())
}

/// Back / forward run as JavaScript in the page: `Webview` exposes no history
/// API, and `history.go` needs no same-origin permission to drive the page's
/// own session history.
#[tauri::command]
pub fn browser_history(app: AppHandle, id: u32, delta: i32) {
    if let Some(webview) = page(&app, id) {
        webview.eval(&format!("history.go({delta})")).ok();
    }
}

#[tauri::command]
pub fn browser_reload(app: AppHandle, id: u32) {
    if let Some(webview) = page(&app, id) {
        webview.eval("location.reload()").ok();
    }
}

/// Closes a tab and its page. A hidden webview still holds a live page — and
/// its audio — so closing the tab has to mean closing the webview. Which
/// tab the strip lands on next is the frontend's call; it hears the change
/// through `browser-state`.
#[tauri::command]
pub fn browser_close_tab(app: AppHandle, id: u32) {
    if let Some(webview) = page(&app, id) {
        webview.close().ok();
    }
    with_state(&app, |s| s.tabs.retain(|t| t.id != id));
    broadcast(&app);
}
