//! A page nobody can see never takes the keyboard.
//!
//! WebKit makes a page first responder whenever its script focuses an
//! element, visible or not: a backgrounded browser tab would take every
//! keystroke and ⌘-key from what is in front, and the page it took them from
//! loses its selection on resigning.
//! The main window refuses `makeFirstResponder:` for a responder inside a page
//! that is hidden or off the window, so the steal never happens.
//!
//! The window is already a KVO class by setup, so it can't be isa-swizzled
//! the way `hit_test.rs` swizzles its content view. The method is replaced on
//! its real class (tao's `TaoWindow`) instead, which the KVO class inherits;
//! every other window's call goes straight to the original.

use std::sync::OnceLock;

use objc2::runtime::{AnyClass, AnyObject, Bool, Imp, Sel};
use objc2::{class, ffi, msg_send, sel};
use objc2_foundation::NSRect;

type MakeFirstResponder = unsafe extern "C-unwind" fn(&AnyObject, Sel, *mut AnyObject) -> Bool;

/// The implementation this replaced, which every allowed call goes to.
static ORIGINAL: OnceLock<MakeFirstResponder> = OnceLock::new();

/// Replaces `makeFirstResponder:` on `window`'s real class. Main thread.
pub(super) unsafe fn install(window: *mut AnyObject) {
    let Some(window) = window.as_ref() else {
        return;
    };
    if ORIGINAL.get().is_some() {
        return;
    }
    let mut class = window.class();
    while class.name().to_bytes().starts_with(b"NSKVONotifying_") {
        let Some(parent) = class.superclass() else {
            return;
        };
        class = parent;
    }
    let name = class.name().to_string_lossy();
    let selector = sel!(makeFirstResponder:);
    let Some(method) = class.instance_method(selector) else {
        elog!("[oculus] native layer: {name} has no makeFirstResponder:; hidden pages can take the keyboard");
        return;
    };
    // SAFETY: NSWindow's `- (BOOL)makeFirstResponder:(NSResponder *)responder`.
    let original: MakeFirstResponder = std::mem::transmute(method.implementation());
    ORIGINAL.get_or_init(|| original);
    let replacement: MakeFirstResponder = make_first_responder;
    ffi::class_replaceMethod(
        class as *const AnyClass as *mut AnyClass,
        selector,
        std::mem::transmute::<MakeFirstResponder, Imp>(replacement),
        ffi::method_getTypeEncoding(method),
    );
}

/// Refused (NO, first responder unchanged) for a responder inside a page
/// nobody can see; everything else goes to the original.
unsafe extern "C-unwind" fn make_first_responder(
    this: &AnyObject,
    cmd: Sel,
    responder: *mut AnyObject,
) -> Bool {
    if in_unseen_page(this, responder) {
        return Bool::NO;
    }
    match ORIGINAL.get() {
        Some(original) => original(this, cmd, responder),
        None => Bool::NO,
    }
}

/// `responder` is, or is inside, a page (a content-view subview other than the
/// app's webview and the glass) that is hidden or lies off the window.
unsafe fn in_unseen_page(window: &AnyObject, responder: *mut AnyObject) -> bool {
    if responder.is_null() {
        return false;
    }
    let is_view: bool = msg_send![responder, isKindOfClass: class!(NSView)];
    if !is_view {
        return false;
    }
    let content: *mut AnyObject = msg_send![window, contentView];
    if content.is_null() {
        return false;
    }
    let subviews: *mut AnyObject = msg_send![content, subviews];
    if subviews.is_null() {
        return false;
    }
    let app_view = crate::shell::keys::app_webview() as *mut AnyObject;
    let glass = super::glass::view();
    let count: usize = msg_send![subviews, count];
    for i in 0..count {
        let page: *mut AnyObject = msg_send![subviews, objectAtIndex: i];
        if page.is_null() || page == app_view || page == glass {
            continue;
        }
        let inside = responder == page || {
            let descendant: bool = msg_send![responder, isDescendantOf: page];
            descendant
        };
        if inside {
            return !shown(content, page);
        }
    }
    false
}

/// Not hidden and overlapping the content view; background browser tabs are hidden.
unsafe fn shown(content: *mut AnyObject, page: *mut AnyObject) -> bool {
    let hidden: bool = msg_send![page, isHiddenOrHasHiddenAncestor];
    if hidden {
        return false;
    }
    let frame: NSRect = msg_send![page, frame];
    let bounds: NSRect = msg_send![content, bounds];
    frame.origin.x < bounds.origin.x + bounds.size.width
        && bounds.origin.x < frame.origin.x + frame.size.width
        && frame.origin.y < bounds.origin.y + bounds.size.height
        && bounds.origin.y < frame.origin.y + frame.size.height
}
