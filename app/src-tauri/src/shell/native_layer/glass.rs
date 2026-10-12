//! The glass frame: an `NSVisualEffectView` at the very bottom of the main
//! window's content view, blurring the desktop behind the window. Where the
//! frontend's ground on `#root` is translucent, it shows through the app's
//! transparent webview; a solid frame paints over it and hides it
//! (`native_set_glass`). Pages sit above it, the app's webview above them.
//!
//! Raw `msg_send!`, as in `hit_test.rs`: the typed AppKit views would need
//! more `objc2-app-kit` features.

use std::sync::atomic::{AtomicPtr, Ordering};

use objc2::rc::Retained;
use objc2::runtime::AnyObject;
use objc2::{class, msg_send};
use objc2_foundation::{ns_string, NSRect};
use tauri::AppHandle;

/// The effect view, retained for the life of the app; null until `install`.
static GLASS: AtomicPtr<AnyObject> = AtomicPtr::new(std::ptr::null_mut());

/// `NSVisualEffectBlendingModeBehindWindow`.
const BEHIND_WINDOW: isize = 0;
/// `NSVisualEffectMaterialUnderWindowBackground`.
const UNDER_WINDOW_BACKGROUND: isize = 21;
/// `NSVisualEffectStateActive`: stays vibrant while the window is inactive.
const STATE_ACTIVE: isize = 1;
/// `NSViewWidthSizable | NSViewHeightSizable`.
const FILL_SUPERVIEW: usize = 2 | 16;
/// `NSWindowBelow`; with a nil sibling, below every other subview.
const NS_WINDOW_BELOW: isize = -1;

/// Makes the window non-opaque and puts the effect view under everything in
/// `app_view`'s superview. Main thread, from `with_webview`, before any page.
pub(super) unsafe fn install(app_view: *mut AnyObject) {
    if app_view.is_null() || !GLASS.load(Ordering::Relaxed).is_null() {
        return;
    }
    let content: *mut AnyObject = msg_send![app_view, superview];
    let window: *mut AnyObject = msg_send![app_view, window];
    if content.is_null() || window.is_null() {
        elog!("[oculus] native layer: the app's webview has no window yet; no glass frame");
        return;
    }
    let clear: *mut AnyObject = msg_send![class!(NSColor), clearColor];
    let _: () = msg_send![window, setOpaque: false];
    let _: () = msg_send![window, setBackgroundColor: clear];

    let glass: Option<Retained<AnyObject>> = msg_send![class!(NSVisualEffectView), new];
    let Some(glass) = glass else {
        elog!("[oculus] native layer: could not make an NSVisualEffectView; no glass frame");
        return;
    };
    let bounds: NSRect = msg_send![content, bounds];
    let _: () = msg_send![&*glass, setFrame: bounds];
    let _: () = msg_send![&*glass, setAutoresizingMask: FILL_SUPERVIEW];
    let _: () = msg_send![&*glass, setBlendingMode: BEHIND_WINDOW];
    let _: () = msg_send![&*glass, setMaterial: UNDER_WINDOW_BACKGROUND];
    let _: () = msg_send![&*glass, setState: STATE_ACTIVE];
    let nil: *mut AnyObject = std::ptr::null_mut();
    let _: () = msg_send![
        content,
        addSubview: &*glass,
        positioned: NS_WINDOW_BELOW,
        relativeTo: nil,
    ];
    GLASS.store(Retained::into_raw(glass), Ordering::Relaxed);
}

/// The effect view, or null when `install` didn't run or failed.
pub(super) fn view() -> *mut AnyObject {
    GLASS.load(Ordering::Relaxed)
}

/// Shows or hides the glass and gives it the app's appearance, which can
/// differ from the OS's.
pub(super) fn set(app: &AppHandle, enabled: bool, dark: bool) {
    let result = app.run_on_main_thread(move || unsafe {
        let glass = view();
        if glass.is_null() {
            return;
        }
        let name = if dark {
            ns_string!("NSAppearanceNameDarkAqua")
        } else {
            ns_string!("NSAppearanceNameAqua")
        };
        let appearance: *mut AnyObject = msg_send![class!(NSAppearance), appearanceNamed: name];
        let _: () = msg_send![glass, setAppearance: appearance];
        let _: () = msg_send![glass, setHidden: !enabled];
    });
    if let Err(e) = result {
        elog!("[oculus] native layer: glass frame: {e}");
    }
}
