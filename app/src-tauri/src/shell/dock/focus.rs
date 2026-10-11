//! A parked page keeps first responder unless told otherwise, and then every
//! keystroke goes to a page nobody can see. Parking or closing a page that
//! holds focus hands it to the app's own webview.

use tauri::webview::Webview;

/// Raw `msg_send!`: the typed `NSView`/`NSWindow` API would pull in AppKit
/// features for a handful of calls.
#[cfg(target_os = "macos")]
pub(super) fn release(page: &Webview<tauri::Wry>) {
    use objc2::runtime::AnyObject;
    use objc2::{class, msg_send};

    let app_view = crate::shell::keys::app_webview();
    page.with_webview(move |platform| unsafe {
        let view = platform.inner() as *mut AnyObject;
        if view.is_null() {
            return;
        }
        let window: *mut AnyObject = msg_send![view, window];
        if window.is_null() {
            return;
        }
        let responder: *mut AnyObject = msg_send![window, firstResponder];
        if responder.is_null() {
            return;
        }
        // WebKit can make one of its own subviews first responder.
        let inside = responder == view || {
            let is_view: bool = msg_send![responder, isKindOfClass: class!(NSView)];
            is_view && {
                let descendant: bool = msg_send![responder, isDescendantOf: view];
                descendant
            }
        };
        if inside {
            // Nil (before `keys::install` ran) leaves the window itself first
            // responder, which is still not the parked page.
            let target = app_view as *mut AnyObject;
            let _: bool = msg_send![window, makeFirstResponder: target];
        }
    })
    .ok();
}

#[cfg(not(target_os = "macos"))]
pub(super) fn release(page: &Webview<tauri::Wry>) {
    use tauri::Manager;

    if let Some(app) = page.app_handle().get_webview(crate::shell::browser::MAIN) {
        app.set_focus().ok();
    }
}
