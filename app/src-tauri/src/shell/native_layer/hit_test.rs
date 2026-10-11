//! The `hitTest:` override on the main window's content view (tao's
//! `TaoView`, which holds the app's webview and every page as siblings).
//!
//! That one instance is isa-swizzled to a subclass made at runtime. The
//! WKWebView itself is left alone: KVO may have swizzled it already, and
//! dropping its last observer restores the old class and our override with
//! it. Raw `msg_send!`: the typed `NSView` would need more AppKit features.

use std::ffi::CString;
use std::sync::atomic::{AtomicPtr, Ordering};
use std::sync::OnceLock;

use objc2::runtime::{AnyClass, AnyObject, ClassBuilder, Sel};
use objc2::{msg_send, sel};
use objc2_foundation::{ns_string, NSNumber, NSPoint, NSRect};

/// The app's own WKWebView, which the override looks past.
static APP_VIEW: AtomicPtr<AnyObject> = AtomicPtr::new(std::ptr::null_mut());
/// The content view's original class, which `super` calls go to.
static BASE: OnceLock<&'static AnyClass> = OnceLock::new();

const SUBCLASS_PREFIX: &str = "OculusPageHitTest_";

/// Turns off the app webview's background and overrides its superview's
/// `hitTest:`. Main thread, from `with_webview`.
pub(super) unsafe fn install(app_view: *mut AnyObject) {
    if app_view.is_null() {
        return;
    }
    APP_VIEW.store(app_view, Ordering::Relaxed);
    // The private KVC key wry's own `transparent` feature sets.
    let no = NSNumber::numberWithBool(false);
    let _: () = msg_send![app_view, setValue: &*no, forKey: ns_string!("drawsBackground")];

    let content: *mut AnyObject = msg_send![app_view, superview];
    let Some(content) = content.as_ref() else {
        elog!("[oculus] native layer: the app's webview has no superview; clicks stay with it");
        return;
    };
    let class = content.class();
    let name = class.name().to_string_lossy();
    if name.starts_with(SUBCLASS_PREFIX) {
        return;
    }
    // A KVO class would swap itself back when its last observer goes.
    if name.starts_with("NSKVONotifying_") {
        elog!("[oculus] native layer: content view is {name}; clicks stay with the app's webview");
        return;
    }
    let Some(subclass) = subclass_of(class) else {
        elog!(
            "[oculus] native layer: could not subclass {name}; clicks stay with the app's webview"
        );
        return;
    };
    AnyObject::set_class(content, subclass);
}

/// `OculusPageHitTest_<base>`, made once. Adds no ivars, so the instance size
/// matches and the swizzle is sound.
fn subclass_of(base: &'static AnyClass) -> Option<&'static AnyClass> {
    if !std::ptr::eq(*BASE.get_or_init(|| base), base) {
        return None;
    }
    let name = CString::new(format!(
        "{SUBCLASS_PREFIX}{}",
        base.name().to_string_lossy()
    ))
    .ok()?;
    if let Some(existing) = AnyClass::get(&name) {
        return Some(existing);
    }
    let mut builder = ClassBuilder::new(&name, base)?;
    // SAFETY: NSView's `- (NSView *)hitTest:(NSPoint)point` and NSObject's
    // `- (Class)superclass`.
    unsafe {
        builder.add_method(
            sel!(hitTest:),
            hit_test as unsafe extern "C-unwind" fn(_, _, _) -> _,
        );
        builder.add_method(
            sel!(superclass),
            superclass as unsafe extern "C-unwind" fn(_, _) -> _,
        );
    }
    Some(builder.register())
}

/// The base's superclass, as KVO's subclasses do. tao's `drawRect:` calls
/// `super` through `[self superclass]`; answering with the base would send it
/// back into its own `drawRect:` until the stack overflows.
unsafe extern "C-unwind" fn superclass(_this: &AnyObject, _cmd: Sel) -> *const AnyClass {
    BASE.get()
        .and_then(|base| base.superclass())
        .map_or(std::ptr::null(), |class| class as *const AnyClass)
}

/// `point` is in the content view's superview's coordinates (NSView's
/// contract). A visible page under it takes the click unless an overlay is
/// open; anything else falls through to NSView's own walk, which finds the
/// app's webview.
unsafe extern "C-unwind" fn hit_test(
    this: &AnyObject,
    _cmd: Sel,
    point: NSPoint,
) -> *mut AnyObject {
    if !super::blocked() {
        if let Some(hit) = page_at(this, point) {
            return hit;
        }
    }
    match BASE.get() {
        Some(base) => msg_send![super(this, base), hitTest: point],
        None => std::ptr::null_mut(),
    }
}

/// The deepest view of the top-most visible page containing `point`. Hidden
/// browser pages are `isHidden`; parked dock pages sit off the window.
unsafe fn page_at(this: &AnyObject, point: NSPoint) -> Option<*mut AnyObject> {
    let app_view = APP_VIEW.load(Ordering::Relaxed);
    let parent: *mut AnyObject = msg_send![this, superview];
    let local: NSPoint = msg_send![this, convertPoint: point, fromView: parent];
    let subviews: *mut AnyObject = msg_send![this, subviews];
    if subviews.is_null() {
        return None;
    }
    let count: usize = msg_send![subviews, count];
    for i in (0..count).rev() {
        let sub: *mut AnyObject = msg_send![subviews, objectAtIndex: i];
        if sub.is_null() || sub == app_view {
            continue;
        }
        let hidden: bool = msg_send![sub, isHidden];
        if hidden {
            continue;
        }
        let frame: NSRect = msg_send![sub, frame];
        if !contains(frame, local) {
            continue;
        }
        // A subview's `hitTest:` takes its superview's coordinates: ours.
        let hit: *mut AnyObject = msg_send![sub, hitTest: local];
        if !hit.is_null() {
            return Some(hit);
        }
    }
    None
}

fn contains(frame: NSRect, p: NSPoint) -> bool {
    p.x >= frame.origin.x
        && p.x < frame.origin.x + frame.size.width
        && p.y >= frame.origin.y
        && p.y < frame.origin.y + frame.size.height
}
