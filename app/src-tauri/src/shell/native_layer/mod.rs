//! How native browser pages share the window with the
//! app's webview, so popovers draw over a live page.
//!
//! On macOS every page is a child WKWebView of the main window, a sibling of
//! the app's own, and pages sit under the app. The content view's subviews
//! run, bottom to top: the glass frame (`glass.rs`), the pages, the app's
//! webview. `adopt_page` moves each new page to just above the glass, and
//! `init` stops the app's webview drawing a background, so the frontend can
//! cut a hole in its DOM where a page sits and let the glass show through a
//! translucent ground. `native_set_glass` shows or hides the glass and gives
//! it the app's light or dark appearance. The app's webview covers the whole
//! window, so on its own it would take every click. `init` overrides
//! `hitTest:` on the window's content view (`hit_test.rs`): a click inside a
//! visible page's frame goes to that page, unless the frontend has said an
//! overlay is open (`native_set_blocked`). The window also refuses first
//! responder to a page nobody can see, so a hidden page's script can't take
//! the keyboard (`focus_guard.rs`).
//!
//! On Windows pages stay on top and the app's webview stays opaque; each
//! page is clipped to the frontend's holes minus the portals over them by a
//! window region (`windows.rs`, `native_set_holes`), and clicks follow it.

#[cfg(target_os = "macos")]
mod focus_guard;
#[cfg(windows)]
mod gdi;
#[cfg(target_os = "macos")]
mod glass;
#[cfg(target_os = "macos")]
mod hit_test;
pub(crate) mod page_theme;
pub(crate) use page_theme::apply as apply_page_theme;
pub use page_theme::PageTheme;
#[cfg(any(windows, test))]
mod region;
#[cfg(windows)]
pub(crate) mod windows;

#[cfg(windows)]
pub use windows::{adopt_page, init, refresh_page};

use std::sync::atomic::{AtomicBool, Ordering};

#[cfg(not(windows))]
use tauri::webview::Webview;
#[cfg(not(windows))]
use tauri::AppHandle;

/// An overlay covers the pages: the app's webview keeps every click.
static BLOCKED: AtomicBool = AtomicBool::new(false);

/// The frontend sets this while a popover, menu or dialog is open.
#[tauri::command]
pub fn native_set_blocked(blocked: bool) {
    BLOCKED.store(blocked, Ordering::Relaxed);
}

/// The frontend calls this whenever the frame or the theme changes: `enabled`
/// is a glass frame, `dark` the app's theme. Only macOS has the glass.
#[tauri::command]
pub fn native_set_glass(app: tauri::AppHandle, enabled: bool, dark: bool) {
    #[cfg(target_os = "macos")]
    glass::set(&app, enabled, dark);
    #[cfg(not(target_os = "macos"))]
    let _ = (app, enabled, dark);
}

#[cfg(target_os = "macos")]
fn blocked() -> bool {
    BLOCKED.load(Ordering::Relaxed)
}

/// Makes the app's webview transparent and installs the glass frame, the
/// click routing and the focus guard. From `setup`, before any page exists.
#[cfg(target_os = "macos")]
pub fn init(app: &AppHandle) {
    use tauri::Manager;

    use super::browser::MAIN;

    let Some(webview) = app.get_webview(MAIN) else {
        elog!("[oculus] native layer: no main webview at setup; pages will cover the app");
        return;
    };
    if let Err(e) = webview.with_webview(|platform| unsafe {
        let view: *mut objc2::runtime::AnyObject = platform.inner().cast();
        hit_test::install(view);
        glass::install(view);
        if !view.is_null() {
            focus_guard::install(objc2::msg_send![view, window]);
        }
    }) {
        elog!("[oculus] native layer: {e}");
    }
}

#[cfg(not(any(target_os = "macos", windows)))]
pub fn init(_app: &AppHandle) {}

/// Moves a just-added page to just above the glass, under the app's webview
/// and every older page; to the bottom without the glass. Called right after
/// `add_child`.
#[cfg(target_os = "macos")]
pub fn adopt_page(webview: &Webview<tauri::Wry>) {
    use objc2::msg_send;
    use objc2::rc::Retained;
    use objc2::runtime::AnyObject;

    /// `NSWindowBelow`; with a nil sibling, below every other subview.
    const NS_WINDOW_BELOW: isize = -1;
    /// `NSWindowAbove`; with a sibling, directly above it.
    const NS_WINDOW_ABOVE: isize = 1;

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
            let glass = glass::view();
            let glass_parent: *mut AnyObject = if glass.is_null() {
                std::ptr::null_mut()
            } else {
                msg_send![glass, superview]
            };
            let (order, sibling) = if !glass.is_null() && glass_parent == parent {
                (NS_WINDOW_ABOVE, glass)
            } else {
                (NS_WINDOW_BELOW, std::ptr::null_mut())
            };
            let _: () = msg_send![
                parent,
                addSubview: view,
                positioned: order,
                relativeTo: sibling,
            ];
        })
        .ok();
}

#[cfg(not(any(target_os = "macos", windows)))]
pub fn adopt_page(_webview: &Webview<tauri::Wry>) {}

/// After Rust moves or resizes a page. Only Windows clips pages itself.
#[cfg(not(windows))]
pub fn refresh_page(_webview: &Webview<tauri::Wry>) {}
