//! The messaging dock: WhatsApp, Instagram, Messenger and Discord as native
//! child WKWebViews of the main window, one persistent WebKit data store each.
//!
//! - **A page lives from enable to disable.** It is never recreated to show
//!   it, and the frontend owns the enabled set (it calls `dock_set_enabled`).
//! - **Shown = placed in its slot. Parked = off-screen at the slot's size.**
//!   Never `hide()`, which throttles timers to ~1 Hz, and never a sliver,
//!   which flips these sites to their mobile layouts. Placing never takes
//!   keyboard focus; parking hands it back to the app (`focus.rs`).
//! - **Separate from the browser.** Labels are `dock-<id>`, state goes out
//!   as `dock-state`, and no dock page ever enters `BrowserState`.
//! - **No capability may reach these pages**: they hold remote content, and
//!   `capabilities_test.rs` enforces it. See `docs/dock.md`.
//! - **Pages report only through the `dock` message handler** (`channel.rs`),
//!   and only into the in-memory Inbox (`inbox.rs`). Unread counts come from
//!   page titles (`title.rs`).

#[cfg(test)]
mod capabilities_test;
mod channel;
pub(crate) mod commands;
mod focus;
mod inbox;
mod layout;
mod message;
mod page;
mod scripts;
mod services;
#[cfg(test)]
mod tests;
mod title;

pub use inbox::InboxItem;
pub use layout::init;

use std::collections::HashMap;
use std::sync::Mutex;

use serde::Serialize;
use tauri::{AppHandle, Emitter, EventTarget, Manager};

use super::browser::{Viewport, MAIN};
use services::SERVICES;

pub const LABEL_PREFIX: &str = "dock-";

fn label(id: &str) -> String {
    format!("{LABEL_PREFIX}{id}")
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DockPageInfo {
    pub id: String,
    pub title: String,
    pub url: String,
    pub loading: bool,
    /// The title's leading `(N)`; 0 without one.
    pub unread: u32,
}

/// One entry per created (enabled) page, in `SERVICES` order.
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DockSnapshot {
    pub pages: Vec<DockPageInfo>,
    pub shown: Option<String>,
}

#[derive(Default)]
struct Inner {
    pages: Vec<DockPageInfo>,
    /// Each page's last slot, kept while it is parked so it parks at that size.
    viewports: HashMap<String, Viewport>,
    /// The most recent slot of any page, for a page parked before its first
    /// placement: every dock page sits in the same content card.
    last_viewport: Option<Viewport>,
    shown: Option<String>,
    inbox: inbox::Inbox,
}

#[derive(Default)]
pub struct DockState(Mutex<Inner>);

fn with_state<T>(app: &AppHandle, f: impl FnOnce(&mut Inner) -> T) -> T {
    let state = app.state::<DockState>();
    let mut inner = state.0.lock().unwrap_or_else(|e| e.into_inner());
    f(&mut inner)
}

fn snapshot(app: &AppHandle) -> DockSnapshot {
    with_state(app, |s| {
        let mut pages = s.pages.clone();
        pages.sort_by_key(|p| SERVICES.iter().position(|svc| svc.id == p.id));
        DockSnapshot {
            pages,
            shown: s.shown.clone(),
        }
    })
}

/// Targeted at the app's webview only, never at the pages themselves.
fn broadcast(app: &AppHandle) {
    app.emit_to(EventTarget::webview(MAIN), "dock-state", snapshot(app))
        .ok();
}
