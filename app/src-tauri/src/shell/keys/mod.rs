//! ⌘-keys reach a focused browser page before the menu, as in Safari.
//!
//! Every webview is a child webview, and wry's `WryWebView` answers NO to
//! `performKeyEquivalent:` for those, so a page never sees a ⌘-key. A local
//! key-down monitor calls WebKit's own implementation when a page is first
//! responder. WebKit answers YES and never hands a key the page leaves back
//! to the menu, so `fallback.rs` does: the page reports the keydown it didn't
//! cancel, and the menu gets that event. The app's own webview is left alone,
//! and so are the `RESERVED` chrome keys. No webview goes back on Backspace
//! (`backspace.rs`).

mod backspace;
mod fallback;

pub(crate) use fallback::{install as install_fallback, SCRIPT as FALLBACK_SCRIPT};

use std::sync::atomic::{AtomicUsize, Ordering};

use block2::RcBlock;
use objc2::rc::Retained;
use objc2::runtime::{AnyObject, Bool};
use objc2::{class, msg_send, ClassType, MainThreadMarker};
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
            .with_webview(|pv| {
                let ptr = pv.inner() as *mut WKWebView;
                APP_WEBVIEW.store(ptr as usize, Ordering::Relaxed);
                if let Some(view) = unsafe { ptr.as_ref() } {
                    backspace::disable(view);
                }
            })
            .ok();
    }

    let handler = RcBlock::new(|event: std::ptr::NonNull<NSEvent>| -> *mut NSEvent {
        let event_ref = unsafe { event.as_ref() };
        if !event_ref
            .modifierFlags()
            .contains(NSEventModifierFlags::Command)
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
        // WebKit hands the key to the page and answers YES; a key the page
        // leaves comes back through `fallback`.
        let handled: Bool = unsafe {
            msg_send![super(&*page, WKWebView::class()), performKeyEquivalent: event_ref]
        };
        if handled.as_bool() {
            fallback::remember(event_ref, &page);
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

/// The app's own `WKWebView` as an address, 0 until `install` has run.
/// The native focus guard distinguishes it from background browser pages.
pub(crate) fn app_webview() -> usize {
    APP_WEBVIEW.load(Ordering::Relaxed)
}

/// The browser page holding keyboard focus: the `WKWebView` other than the
/// app's own that is the key window's first responder or, as WebKit sometimes
/// makes one of its own subviews first responder, the one that contains it.
pub fn focused_page(mtm: MainThreadMarker) -> Option<Retained<NSResponder>> {
    let app_webview = APP_WEBVIEW.load(Ordering::Relaxed);
    if app_webview == 0 {
        return None;
    }
    let responder = NSApplication::sharedApplication(mtm)
        .keyWindow()?
        .firstResponder()?;
    // Raw `msg_send!`: the typed `NSView` API would need another AppKit feature.
    let mut view = Retained::as_ptr(&responder) as *mut AnyObject;
    while !view.is_null() {
        let is_view: bool = unsafe { msg_send![view, isKindOfClass: class!(NSView)] };
        if !is_view {
            return None;
        }
        let is_page: bool = unsafe { msg_send![view, isKindOfClass: WKWebView::class()] };
        if is_page {
            return (view as usize != app_webview)
                .then(|| unsafe { Retained::retain(view as *mut NSResponder) })
                .flatten();
        }
        view = unsafe { msg_send![view, superview] };
    }
    None
}
