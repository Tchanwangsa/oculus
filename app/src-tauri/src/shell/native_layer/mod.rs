//! Native pages (browser tabs, dock services) sit under the app's webview.
//!
//! Every page is a child WKWebView of the main window, a sibling of the app's
//! own. `put_below_app` moves each new page to the bottom of that stack, and
//! `init` stops the app's webview drawing a background, so the frontend can
//! cut a hole in its DOM where a page sits and its popovers draw over the
//! live page.
//!
//! The app's webview covers the whole window, so on its own it would take
//! every click. `init` overrides `hitTest:` on the window's content view
//! (`hit_test.rs`): a click inside a visible page's frame goes to that page,
//! unless the frontend has said an overlay is open (`native_set_blocked`).

#[cfg(target_os = "macos")]
mod hit_test;

use std::sync::atomic::{AtomicBool, Ordering};

use tauri::webview::Webview;
use tauri::AppHandle;

/// An overlay covers the pages: the app's webview keeps every click.
static BLOCKED: AtomicBool = AtomicBool::new(false);

/// The frontend sets this while a popover, menu or dialog is open.
#[tauri::command]
pub fn native_set_blocked(blocked: bool) {
    BLOCKED.store(blocked, Ordering::Relaxed);
}

fn blocked() -> bool {
    BLOCKED.load(Ordering::Relaxed)
}

/// Makes the app's webview transparent and installs the click routing.
/// From `setup`, before any page exists.
#[cfg(target_os = "macos")]
pub fn init(app: &AppHandle) {
    use tauri::Manager;

    use super::browser::MAIN;

    let Some(webview) = app.get_webview(MAIN) else {
        elog!("[oculus] native layer: no main webview at setup; pages will cover the app");
        return;
    };
    if let Err(e) = webview.with_webview(|platform| unsafe {
        hit_test::install(platform.inner().cast());
    }) {
        elog!("[oculus] native layer: {e}");
    }
}

#[cfg(not(target_os = "macos"))]
pub fn init(_app: &AppHandle) {}

/// Moves a just-added page to the bottom of the window's views, under the
/// app's webview. Called right after `add_child`.
#[cfg(target_os = "macos")]
pub fn put_below_app(webview: &Webview<tauri::Wry>) {
    use objc2::msg_send;
    use objc2::rc::Retained;
    use objc2::runtime::AnyObject;

    /// `NSWindowBelow`; with a nil sibling, below every other subview.
    const NS_WINDOW_BELOW: isize = -1;

    webview
        .with_webview(|platform| unsafe {
            let view: *mut AnyObject = platform.inner().cast();
            if view.is_null() {
                return;
            }
            let parent: *mut AnyObject = msg_send![view, superview];
            if parent.is_null() {
                return;
            }
            // Re-adding detaches the view for a moment; hold it across that.
            let Some(_held) = Retained::retain(view) else {
                return;
            };
            let nil: *mut AnyObject = std::ptr::null_mut();
            let _: () = msg_send![
                parent,
                addSubview: view,
                positioned: NS_WINDOW_BELOW,
                relativeTo: nil,
            ];
        })
        .ok();
}

#[cfg(not(target_os = "macos"))]
pub fn put_below_app(_webview: &Webview<tauri::Wry>) {}
