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
//! - **The frontend says which pages are on screen and where, Rust puts
//!   them there.** A native view cannot interleave with the DOM, so a page
//!   lives in a slot the React tree leaves for it and the frontend reports
//!   that slot as insets from the window's edges (`Viewport`). Insets, not a
//!   rect: a window resize is then laid out here from the window size alone,
//!   with no JavaScript in the loop to lag behind it. Slot and visibility are
//!   both per page — a page in the content card and a page in a side panel
//!   are on screen together — so Rust never takes showing one to mean hiding
//!   another; it does what it is told, page by page. The frontend speaks up
//!   when a page's insets change — sidebar, panel drag, zoom — and when
//!   something of its own has to draw over a page, which is the one thing a
//!   native view cannot allow: it asks for that page to be hidden until the
//!   popup is gone.
//!
//! The toolbar is the fourth: **what a page knows about itself, only the
//! page can say.** Whether the back list has anywhere to go, what a find
//! matched, what the zoom is — none of that is derivable from the URL, and
//! Tauri exposes no API for the first two. They are read and driven on the
//! WKWebView directly (`nav_state`, `find_string`), which means they arrive
//! *late*: `with_webview` dispatches to the main thread and hands back
//! nothing, so the answer is written into the tab and broadcast from inside
//! the callback rather than returned to a command. Favicons are the same
//! shape for a different reason — WebKit has no public icon API at all, so
//! the icon is fetched over HTTP beside the page, once per host, and pushed
//! out on its own event so it never rides in the snapshot.
//!
//! Signed in, not for free: `canvas_session` is HttpOnly *and* session-scoped,
//! so WebKit holds it in memory only and it is gone when the app quits — the
//! scraper's snapshot on disk is the only copy that survives.
//! `seed_canvas_session` puts it back into WebKit's jar before a Canvas page
//! loads. (`/login/session_token`, the API built for exactly this, answers
//! 403 here: it wants an access token, and UniMelb has those disabled — see
//! `docs/auth.md`.) Browsing Canvas then rolls the session forward, so every
//! Canvas page load re-snapshots the cookie and the scraper inherits it.

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

/// What a page tells the web it is.
///
/// Without this a page webview sends WKWebView's bare default, which stops at
/// `AppleWebKit/605.1.15 (KHTML, like Gecko)` — no `Version/… Safari/…` suffix,
/// because nothing set `applicationNameForUserAgent`. Sites that sniff the UA
/// read that as an engine they have never heard of and serve their fallback:
/// google.com answers a no-JavaScript page of 86 KB instead of the real one at
/// 221 KB, which is the 2004-looking grey-button Google this constant exists to
/// stop. It is not a rendering problem — the engine is Safari's either way, so
/// claiming Safari is true in kind and only the version number is a guess.
///
/// Keep the version roughly current (Safari 26.3 here); a stale one is read as
/// an old browser and some sites start degrading again. The scraper's HTTP
/// client has a string of its own for the same reason (`okta.rs`) — they are
/// deliberately not shared, since bumping this one must not disturb a working
/// SSO flow.
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
    /// Whether the page's own back/forward list has anywhere left to go.
    /// Read off the WKWebView after every page load: a URL says nothing
    /// about the history behind it, so an arrow with no answer to this can
    /// only lie in one direction or the other.
    pub can_back: bool,
    pub can_forward: bool,
    /// Page zoom, where 1.0 is 100% — the WKWebView's own `pageZoom`, which
    /// is what a browser's ⌘+ does. Per tab and for the tab's life only: a
    /// zoom remembered per site is a second store to keep true, and this one
    /// is a reading aid, not a preference.
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

/// A site's icon, pushed on its own event rather than folded into the
/// snapshot: the snapshot goes out on every page-load edge, and an icon is
/// kilobytes that would ride along with each one for nothing.
#[derive(Clone, Serialize)]
struct FaviconFound {
    host: String,
    /// A `data:` URL — the bytes, so the frontend renders it without a second
    /// fetch and without the page's own network identity.
    icon: String,
}

/// What a find landed on, for the find bar that asked.
#[derive(Clone, Serialize)]
struct FindResult {
    id: u32,
    query: String,
    found: bool,
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
    /// Where each page goes, as the frontend last reported it. A tab is
    /// absent until its page has been placed once — it is hidden until
    /// then — and drops out again when the tab closes.
    viewports: HashMap<u32, Viewport>,
    /// Hosts whose favicon has been looked for, found or not. One attempt per
    /// host per run: a site with no icon must not cost two HTTP requests on
    /// every page load, and the frontend keeps what was found in the database
    /// so a restart is the only thing that asks again.
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

/// The tab a page webview belongs to, read back out of its label.
fn tab_id(label: &str) -> Option<u32> {
    label.strip_prefix(LABEL_PREFIX)?.parse().ok()
}

/// Every page webview, whatever tab it belongs to.
fn pages(app: &AppHandle) -> Vec<Webview<tauri::Wry>> {
    app.webviews()
        .into_iter()
        .filter(|(label, _)| label.starts_with(LABEL_PREFIX))
        .map(|(_, webview)| webview)
        .collect()
}

/// The window's content area in logical points. Measured once per layout
/// pass: every page's rect is cut out of the same window.
fn content_size(window: &Window) -> LogicalSize<f64> {
    let scale = window.scale_factor().unwrap_or(1.0);
    window
        .inner_size()
        .map(|s| s.to_logical::<f64>(scale))
        .unwrap_or_else(|_| LogicalSize::new(1480.0, 920.0))
}

/// One page's rect in logical points: the window's content area minus the
/// insets its tab reported. A page with no insets yet — just created, never
/// placed — takes the whole window; nothing is visible then, so it only has
/// to be somewhere.
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

/// Puts every placed page back in its own slot. Insets hang off the window's
/// edges, so a resize moves all of them at once — this is the resize handler,
/// and it reads the window and the slots once for the whole pass. A page the
/// frontend has never placed is skipped: it is hidden, and there is no slot
/// to put it back in.
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

/// Puts one page in its slot, for when only that page moved.
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
        .user_agent(PAGE_USER_AGENT)
        // A page webview is a *child* view inside the main window, so it has
        // no window of its own and WebKit reports `outerWidth`/`outerHeight`
        // as 0. That zero is load-bearing for anyone who renders to a canvas:
        // `outerWidth / innerWidth` is the usual way to detect browser zoom,
        // and a degenerate ratio makes a renderer fall back to its minimum
        // scale. Google Docs backed an 816x1056 CSS page tile with a 408x528
        // canvas — a quarter of the resolution a 2x display wants, stretched
        // 4x on the way to the screen, while its DOM chrome stayed crisp and
        // made it look like the app was at fault. Reporting the viewport's
        // own size is what a full-window browser would roughly say anyway.
        // Measured: with this, the same tile comes back 1632x2112.
        .initialization_script(
            r#"(function(){try{var d=function(k,s){Object.defineProperty(window,k,{configurable:true,get:function(){return window[s];}});};d('outerWidth','innerWidth');d('outerHeight','innerHeight');}catch(e){}})()"#,
        )
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
            // The back/forward list has just moved, whichever edge this is:
            // a commit is what pushes an entry, and a finish is when the page
            // that pushed it is really there. Both are cheap — one property
            // read on the main thread — and asking twice is how the arrows
            // stay right for a page that redirects on arrival.
            refresh_nav(&load_app, id);
            if !started {
                // A Canvas page load rolls the session forward; the cookie the
                // scraper replays is a snapshot, so take a fresh one.
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

    let viewport = with_state(app, |s| s.viewports.get(&id).copied());
    let (position, size, radius) = rect(content_size(&window), viewport);
    let webview = window
        .add_child(builder, position, size)
        .map_err(|e| format!("failed to open page webview: {e}"))?;
    // Commands run on the main thread, so nothing paints between the view
    // appearing and this: it is hidden before its first frame.
    webview.hide().ok();
    round_corners(&webview, radius);
    Ok(())
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
// Three things about a page live in the page and nowhere else: whether its
// back/forward list has anywhere to go, what the zoom is, and what a find
// matched. Tauri has an API for the zoom and none for the other two, so they
// go through the WKWebView.
//
// `with_webview` is the only way in and it hands nothing back — it dispatches
// a closure to the main thread and returns immediately. So every one of these
// is written as a *push*: read the answer inside the closure, put it in the
// tab, broadcast. Nothing here returns a value to its caller, and the frontend
// learns what happened the same way it learns everything else.

/// Reads `canGoBack`/`canGoForward` off the page and broadcasts them.
///
/// Called on both edges of every page load. The read is one Objective-C
/// property each, so asking twice costs nothing and covers the page that
/// redirects the moment it commits.
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

/// Elsewhere there is nothing to ask, so the arrows stay live rather than
/// greying out on a guess — the pre-toolbar behaviour, kept where it is still
/// the only honest one.
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

/// One step along the page's own back/forward list.
///
/// The native call, not `history.go(delta)`: session history is the
/// *webview's*, and a page that has replaced `history` — or is simply a
/// document with no script running — still has a back list WebKit will walk.
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
        // One step at a time is all the toolbar asks for; a longer jump would
        // want `backForwardList` and an item, which no control here offers.
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

/// Elsewhere there is only the script API, which has no cache-ignoring form —
/// so a hard reload is an ordinary one, and says so by doing nothing extra.
#[cfg(not(target_os = "macos"))]
fn reload_page(app: &AppHandle, id: u32, _hard: bool) {
    if let Some(webview) = page(app, id) {
        webview.eval("location.reload()").ok();
    }
}

/// How far a page may be zoomed. The same range the app's own window zoom
/// takes, so the two controls feel like one idea at two scopes.
const ZOOM_MIN: f64 = 0.5;
const ZOOM_MAX: f64 = 3.0;

/// Finds `query` in the page and selects the hit, WebKit's own way.
///
/// `findString:withConfiguration:completionHandler:` is the public search API
/// a WKWebView has: it highlights and scrolls to the match itself, wraps at
/// the end, and answers a `WKFindResult` whose one useful field is whether
/// anything matched. There is **no match count** in that answer, so the find
/// bar can say "no results" and nothing more precise — a counter would mean
/// walking the DOM from script, in a page that is not ours.
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
            // Case-insensitive, like every find bar: someone typing a word
            // into a strip of chrome is not asking about its capitals.
            config.setCaseSensitive(false);
        }
        let reply_app = app.clone();
        let echo = query.clone();
        // The completion handler is not optional here for the same reason it
        // is not in `seed_canvas_session`: WebKit calls whatever it was given.
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

/// Elsewhere: `window.find`, which highlights and scrolls the same way but
/// tells us nothing, so the bar is told the search happened and left to say
/// no more than that.
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
// A native view cannot interleave with the DOM, so the app cannot draw over a
// page: an omnibox dropdown rendered in the frontend lands *beneath* the
// WKWebView. Taking the page down for the length of the dropdown is what that
// used to mean, and it blanked the card the moment you typed a character.
//
// A still is the way out of it. WebKit will render a page's visible area into
// an image — `takeSnapshotWithConfiguration:`, the API a browser's tab
// thumbnails come from — so the frontend paints that image in the slot, takes
// the live page down behind it, and draws an ordinary DOM popover over it,
// with its shadow and its rounded corners and its hover states. The page is
// frozen for as long as the list is up, which is the second or two you spend
// typing an address, and pixel-identical to what was there.
//
// Raw `msg_send!` rather than `objc2-web-kit`'s typed wrapper: the typed
// `takeSnapshotWithConfiguration_completionHandler` hands back an `NSImage`
// and is therefore gated behind a dependency on the whole of AppKit, for one
// call whose result becomes bytes immediately. `round_corners` above reaches
// for the runtime the same way and for the same reason.

/// Hands the waiting command its answer, once. WebKit calls a completion
/// handler exactly once, but the block it lives in is an `Fn` and the send
/// consumes the sender, so the sender sits behind a lock either way.
#[cfg(target_os = "macos")]
fn deliver(
    cell: &Mutex<Option<tokio::sync::oneshot::Sender<Option<Vec<u8>>>>>,
    png: Option<Vec<u8>>,
) {
    if let Some(tx) = cell.lock().ok().and_then(|mut slot| slot.take()) {
        let _ = tx.send(png);
    }
}

/// The snapshot `NSImage` as PNG bytes. TIFF is the only representation an
/// `NSImage` hands over directly; `NSBitmapImageRep` re-encodes it, at the
/// image's own pixel size, so a 2x display's still stays 2x and the page
/// does not go soft the moment the list opens.
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
    // NSBitmapImageFileTypePNG. PNG rather than JPEG because the still stands
    // in for the page itself: text on it is read, not glanced at.
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
        // A nil configuration means the defaults: the visible viewport, after
        // pending screen updates — which is exactly "what is on screen now".
        unsafe {
            let config: *mut AnyObject = std::ptr::null_mut();
            let _: () = msg_send![
                view,
                takeSnapshotWithConfiguration: config,
                completionHandler: &*done,
            ];
        }
    });
    // `with_webview` fails on a page that has gone; nothing will call the
    // block, so the waiting side has to be released here.
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
// WebKit has no public favicon API — the icon a WKWebView draws in Safari
// comes from SPI — so the icon is fetched beside the page, over plain HTTP,
// by host.
//
// Two requests at worst and usually one: `/favicon.ico` is still what the
// large majority of sites serve, and only when that comes back as nothing (or
// as an HTML error page dressed as a 200, which is why the bytes are sniffed
// rather than trusted) is the document itself fetched for the `<link rel=icon>`
// it declares. The other way round would be correct in one more case and cost
// a full HTML fetch every time.
//
// Once per host per run. What is found goes out as `browser-favicon` and the
// frontend keeps it in the database, so the second run of the app has the
// icons before any page loads.

/// Bigger than this is not a favicon — it is somebody's hero image behind a
/// misconfigured path, and it would be base64'd into an event.
const FAVICON_MAX: usize = 256 * 1024;
/// Enough of a document to hold its `<head>`.
const FAVICON_HTML_MAX: u64 = 512 * 1024;

fn favicon_agent() -> ureq::Agent {
    ureq::AgentBuilder::new()
        .timeout(Duration::from_secs(8))
        // Claim to be the page's own browser: a site that serves a different
        // icon to a bot is a site whose icon we would rather not show.
        .user_agent(PAGE_USER_AGENT)
        .build()
}

/// Looks for `url`'s site icon in the background, unless this host has been
/// asked about already.
fn ensure_favicon(app: &AppHandle, url: &url::Url) {
    let Some(host) = url.host_str().map(str::to_owned) else {
        return;
    };
    // `insert` answers whether it was new, so the claim and the check are one
    // operation and two page loads landing together cannot both go fetching.
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

/// What these bytes actually are, for the `data:` URL's media type.
///
/// Magic numbers first and the header second, because a 200 is not a promise:
/// plenty of servers answer `/favicon.ico` with their HTML 404 page and the
/// site's own content type, which would otherwise become a broken `<img>` in
/// the tab strip. Unrecognised bytes are only accepted if the header at least
/// claims an image.
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

/// The icon a document declares, largest first.
///
/// `rel~="icon"` is the whole-word match, so it takes `rel="icon"` and
/// `rel="shortcut icon"` and leaves `apple-touch-icon` alone — a 180px iOS
/// tile is the wrong picture for a 14px slot, and often a different one.
/// Among what is left, the biggest declared `sizes` wins over document order,
/// since a legacy 16x16 listed first would otherwise beat the SVG below it.
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
        // "any" is what an SVG declares, and it is the one that scales.
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

/// A slot for tab `id` has mounted: this is where its page goes, put it
/// there and show it. Nothing else is touched — whichever other pages the
/// frontend has on screen stay where they are and stay visible.
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

/// Tab `id`'s slot moved or resized in a way the window size does not
/// explain: the sidebar toggled, a panel was dragged, the zoom changed.
/// Placement only — a hidden page stays hidden.
#[tauri::command]
pub fn browser_set_viewport(app: AppHandle, id: u32, viewport: Viewport) {
    with_state(&app, |s| {
        s.viewports.insert(id, viewport);
    });
    layout_tab(&app, id);
}

/// Tab `id`'s slot went away, or the app has to draw over that page. Hidden,
/// not destroyed: a page you come back to is still where you left it.
#[tauri::command]
pub fn browser_hide_tab(app: AppHandle, id: u32) {
    if let Some(page) = page(&app, id) {
        page.hide().ok();
    }
}

/// A still of tab `id`'s page as PNG bytes, for the frontend to paint in the
/// slot while it draws something over it. Raw bytes rather than a data URL:
/// a window-sized 2x still is megabytes, and base64 would add a third of that
/// again to a string the frontend only wants as a blob.
///
/// An error means there is no still to be had — no page, or WebKit declined.
/// The caller's fallback is what it did before stills existed: hide the page.
#[tauri::command]
pub async fn browser_snapshot(app: AppHandle, id: u32) -> Result<tauri::ipc::Response, String> {
    let (tx, rx) = tokio::sync::oneshot::channel();
    snapshot_page(&app, id, tx);
    match rx.await {
        Ok(Some(png)) => Ok(tauri::ipc::Response::new(png)),
        _ => Err("no snapshot".into()),
    }
}

/// Every page off screen at once, for teardown — the app is leaving a state
/// where any of them could be showing and does not want to name them.
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

/// Back / forward, one entry at a time. `Webview` exposes no history API, so
/// this walks the WKWebView's own back/forward list — see `go_history`, and
/// `Tab::can_back` for how the arrows know whether to offer it.
#[tauri::command]
pub fn browser_history(app: AppHandle, id: u32, delta: i32) {
    go_history(&app, id, delta);
}

/// Reload, and — with `hard` — reload ignoring the cache.
///
/// The distinction is not a nicety. `location.reload()`, which this used to
/// be, is cache-obeying, and a page served from WebKit's cache is how a
/// *fixed* bug goes on reproducing: the user-agent change on 2026-09-17 looked
/// broken for twenty minutes because every reload was serving the response
/// fetched under the old one. `reloadFromOrigin` is the revalidating reload,
/// and it exists only on the WKWebView.
#[tauri::command]
pub fn browser_reload(app: AppHandle, id: u32, hard: bool) {
    reload_page(&app, id, hard);
}

/// Page zoom, as a browser's ⌘+ does it — the WKWebView's `pageZoom`, which
/// scales the page's own layout viewport. Not the app's window zoom, which is
/// a different control at a different scope (`AppLayout`), and not a CSS
/// transform on the slot: the page is a native view over a hole in the DOM,
/// and nothing this side can scale it.
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

/// Find `query` in the page and select the next hit. The answer — matched or
/// not — comes back as a `browser-find` event rather than a return value; see
/// `find_string`.
#[tauri::command]
pub fn browser_find(app: AppHandle, id: u32, query: String, backwards: bool) {
    if query.is_empty() {
        browser_find_clear(app, id);
        return;
    }
    find_string(&app, id, query, backwards);
}

/// Drops the find's selection when the bar closes. WebKit's find API has no
/// "unhighlight" of its own — what it leaves behind is an ordinary selection,
/// so clearing the selection is the whole of it.
#[tauri::command]
pub fn browser_find_clear(app: AppHandle, id: u32) {
    if let Some(webview) = page(&app, id) {
        webview
            .eval("try { window.getSelection().removeAllRanges(); } catch {}")
            .ok();
    }
}

/// Closes a tab and its page. A hidden webview still holds a live page — and
/// its audio — so closing the tab has to mean closing the webview. Which
/// tab the strip lands on next is the frontend's call; it hears the change
/// through `browser-state`.
///
/// **`close()` on its own does not stop the page.** wry's `Drop` for a macOS
/// webview is `removeFromSuperview` followed by `retain` — a deliberate leak,
/// with no matching release anywhere in the crate — and `removeFromSuperview`
/// does not stop media. Meanwhile Tauri drops the label from its webview map
/// first, so the handle below is the *last* one that will ever exist. Closing
/// a playing tab that way left a WKWebView pinned alive off screen, still
/// decoding, still holding the audio device, autoplaying whatever came next,
/// with nothing in the app able to reach it again. So the document is torn
/// down here, while there is still something to tear it down with.
#[tauri::command]
pub fn browser_close_tab(app: AppHandle, id: u32) {
    if let Some(webview) = page(&app, id) {
        // Pause first, then navigate: `about:blank` destroys the media
        // elements outright, but a navigation has to commit, and pausing
        // lands immediately. Both are queued on the webview ahead of the
        // close, and the leaked view outlives the drop — which for once
        // works in our favour, since it means the blank page still commits.
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
