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
//! `seed_sessions` restores it and Okta's session from oculus-keyd before any
//! `*.unimelb.edu.au` load, and each signed-in Canvas page hands both back
//! (see `docs/auth.md`).

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
    /// Placed and not hidden since. A UniMelb page is created only once its
    /// cookies are seeded, which can be after its slot mounted.
    shown: HashSet<u32>,
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

pub(crate) mod commands;
mod favicons;
mod layout;
mod page_state;
mod sessions;
mod still;

use favicons::*;
use layout::*;
pub use layout::{init, open_tab};
use page_state::*;
use sessions::*;
pub use sessions::{clear_sessions, seed_sessions};
use still::*;
