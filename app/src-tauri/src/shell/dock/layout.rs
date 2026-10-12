//! Where a dock page sits: in its slot when shown, off-screen at the slot's
//! size when parked. Both follow window resizes.

use tauri::webview::Webview;
use tauri::{AppHandle, LogicalPosition, LogicalSize, Manager, WindowEvent};

use super::{label, with_state, Inner};
use crate::shell::browser::{content_size, rect, Viewport, MAIN};

/// How far left of the window's edge a parked page's right edge sits.
const PARK_GAP: f64 = 4000.0;

/// Hooks the main window's resize. From `setup`.
pub fn init(app: &AppHandle) {
    let Some(window) = app.get_window(MAIN) else {
        elog!("[oculus] dock: no main window at setup; pages will not follow resizes");
        return;
    };
    let events_app = app.clone();
    window.on_window_event(move |event| {
        if matches!(
            event,
            WindowEvent::Resized(_) | WindowEvent::ScaleFactorChanged { .. }
        ) {
            relayout(&events_app);
        }
    });
}

pub(super) fn webview(app: &AppHandle, id: &str) -> Option<Webview<tauri::Wry>> {
    app.get_webview(&label(id))
}

/// The slot a page is placed in or sized by: its own last one, else the
/// latest any dock page had (they all sit in the same card), else none.
pub(super) fn slot(s: &Inner, id: &str) -> Option<Viewport> {
    s.viewports.get(id).copied().or(s.last_viewport)
}

/// A parked page's rect: the slot's size, entirely left of the window, at the
/// slot's height so nothing reflows when it comes back.
pub(super) fn parked_rect(
    window: LogicalSize<f64>,
    viewport: Option<Viewport>,
) -> (LogicalPosition<f64>, LogicalSize<f64>) {
    let (position, size) = rect(window, viewport);
    (
        LogicalPosition::new(-(size.width + PARK_GAP), position.y),
        size,
    )
}

/// Puts a page in its slot. No `show()` or focus: placement only.
pub(super) fn place(app: &AppHandle, id: &str) {
    let (Some(window), Some(page)) = (app.get_window(MAIN), webview(app, id)) else {
        return;
    };
    let viewport = with_state(app, |s| slot(s, id));
    let (position, size) = rect(content_size(&window), viewport);
    page.set_position(position).ok();
    page.set_size(size).ok();
}

/// Moves a page off-screen at its slot's size. It stays visible to WebKit,
/// so its timers and sockets keep running.
pub(super) fn park(app: &AppHandle, id: &str) {
    let (Some(window), Some(page)) = (app.get_window(MAIN), webview(app, id)) else {
        return;
    };
    let viewport = with_state(app, |s| slot(s, id));
    let (position, size) = parked_rect(content_size(&window), viewport);
    page.set_position(position).ok();
    page.set_size(size).ok();
}

/// The resize handler: the shown page back in its slot, the rest parked at
/// the slot's new size.
fn relayout(app: &AppHandle) {
    let (ids, shown) = with_state(app, |s| {
        let ids: Vec<String> = s.pages.iter().map(|p| p.id.clone()).collect();
        (ids, s.shown.clone())
    });
    for id in ids {
        if shown.as_deref() == Some(id.as_str()) {
            place(app, &id);
        } else {
            park(app, &id);
        }
    }
}
