//! ⌘-keys reach a focused browser page before the menu, as in Safari.
//!
//! Every webview is a child webview, and wry's `WryWebView` answers NO to
//! `performKeyEquivalent:` for those, so WebKit's own implementation (page
//! first, then re-send to the menu if the page doesn't handle the key) never
//! runs. A local key-down monitor calls that implementation itself when a
//! browser page (`browser.rs`) is first responder. The app's own webview is
//! left alone, and so are the `RESERVED` chrome keys.

use std::sync::atomic::{AtomicUsize, Ordering};

use block2::RcBlock;
use objc2::rc::Retained;
use objc2::runtime::{Bool, NSObjectProtocol};
use objc2::{msg_send, ClassType, MainThreadMarker};
use objc2_app_kit::{NSApplication, NSEvent, NSEventMask, NSEventModifierFlags, NSResponder};
use objc2_web_kit::WKWebView;
use tauri::{AppHandle, Manager, Runtime};

/// The app's own `WKWebView`; 0 until `install`'s `with_webview` has run.
static APP_WEBVIEW: AtomicUsize = AtomicUsize::new(0);

/// Virtual key codes that stay with the menu under any shift/option combo:
/// T W N Q H M K L , ` and 1–9. Key codes, not characters, so a Thai layout
/// reserves the same physical keys.
const RESERVED: [u16; 19] = [
    0x11, 0x0D, 0x2D, 0x0C, 0x04, 0x2E, 0x28, 0x25, 0x2B, 0x32, // T W N Q H M K L , `
    0x12, 0x13, 0x14, 0x15, 0x17, 0x16, 0x1A, 0x1C, 0x19, // 1–9
];

/// Records the app's webview and installs the monitor for the app's life.
pub fn install<R: Runtime>(app: &AppHandle<R>) {
    if let Some(webview) = app.get_webview("main") {
        webview
            .with_webview(|pv| APP_WEBVIEW.store(pv.inner() as usize, Ordering::Relaxed))
            .ok();
    }

    let handler = RcBlock::new(|event: std::ptr::NonNull<NSEvent>| -> *mut NSEvent {
        let event_ref = unsafe { event.as_ref() };
        if !event_ref.modifierFlags().contains(NSEventModifierFlags::Command)
            || RESERVED.contains(&event_ref.keyCode())
        {
            return event.as_ptr();
        }
        let Some(mtm) = MainThreadMarker::new() else {
            return event.as_ptr();
        };
        let Some(page) = focused_page(mtm) else {
            return event.as_ptr();
        };
        // WebKit hands the key to the page and answers YES; if the page
        // leaves it, WebKit re-sends it flagged, answers NO, and the menu
        // gets it.
        let handled: Bool = unsafe {
            msg_send![super(&*page, WKWebView::class()), performKeyEquivalent: event_ref]
        };
        if handled.as_bool() {
            std::ptr::null_mut()
        } else {
            event.as_ptr()
        }
    });
    let monitor = unsafe {
        NSEvent::addLocalMonitorForEventsMatchingMask_handler(NSEventMask::KeyDown, &handler)
    };
    // Removing the monitor would need this handle; it lives as long as the app.
    std::mem::forget(monitor);
}

/// The browser page holding keyboard focus: the key window's first responder
/// when it is a `WKWebView` other than the app's own.
pub fn focused_page(mtm: MainThreadMarker) -> Option<Retained<NSResponder>> {
    let app_webview = APP_WEBVIEW.load(Ordering::Relaxed);
    if app_webview == 0 {
        return None;
    }
    let responder = NSApplication::sharedApplication(mtm).keyWindow()?.firstResponder()?;
    let is_page = responder.isKindOfClass(WKWebView::class())
        && Retained::as_ptr(&responder) as usize != app_webview;
    is_page.then_some(responder)
}
