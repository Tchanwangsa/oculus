//! The menu gets a ⌘-key the page didn't take.
//!
//! WebKit's own fall-through (re-sending an unhandled key equivalent through
//! `NSApp sendEvent:`) never comes, so `fallback.js` reports each ⌘-keydown
//! the page didn't cancel over the `oculusKeys` WKScriptMessageHandler, and
//! the event `mod.rs` sent that page goes to the main menu. A message only
//! replays the key the user just sent that same page, once, within
//! `MAX_AGE`, so a page can't fire menu items on its own.

use std::cell::RefCell;
use std::time::{Duration, Instant};

use objc2::rc::Retained;
use objc2::runtime::{AnyObject, NSObject, NSObjectProtocol, ProtocolObject};
use objc2::{define_class, msg_send, MainThreadMarker, MainThreadOnly, Message};
use objc2_app_kit::{NSApplication, NSEvent, NSResponder};
use objc2_foundation::NSString;
use objc2_web_kit::{WKScriptMessage, WKScriptMessageHandler, WKUserContentController, WKWebView};
use tauri::webview::Webview;

/// Injected into every frame of every browser page.
pub(crate) const SCRIPT: &str = include_str!("fallback.js");

const HANDLER: &str = "oculusKeys";

/// Longer than a page's keydown handlers take, short enough that a late
/// message can't replay a key the user has moved on from.
const MAX_AGE: Duration = Duration::from_secs(1);

struct Pending {
    event: Retained<NSEvent>,
    page: usize,
    at: Instant,
}

thread_local! {
    /// The last ⌘-key sent to a page; main thread only.
    static PENDING: RefCell<Option<Pending>> = const { RefCell::new(None) };
}

/// Called by the monitor after WebKit took `event` for `page`.
pub(super) fn remember(event: &NSEvent, page: &NSResponder) {
    let pending = Pending {
        event: event.retain(),
        page: page as *const NSResponder as usize,
        at: Instant::now(),
    };
    PENDING.with(|p| *p.borrow_mut() = Some(pending));
}

define_class!(
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[name = "OculusKeyFallback"]
    struct KeyFallback;

    unsafe impl NSObjectProtocol for KeyFallback {}

    unsafe impl WKScriptMessageHandler for KeyFallback {
        #[unsafe(method(userContentController:didReceiveScriptMessage:))]
        fn did_receive(&self, _controller: &WKUserContentController, message: &WKScriptMessage) {
            let Some(sender) = (unsafe { message.webView() }) else {
                return;
            };
            let sender = Retained::as_ptr(&sender) as usize;
            let Some(event) = PENDING.with(|p| {
                let mut p = p.borrow_mut();
                let fresh = p.as_ref().is_some_and(|pending| {
                    pending.page == sender && pending.at.elapsed() < MAX_AGE
                });
                fresh
                    .then(|| p.take())
                    .flatten()
                    .map(|pending| pending.event)
            }) else {
                return;
            };
            replay(self.mtm(), &event);
        }
    }
);

impl KeyFallback {
    fn new(mtm: MainThreadMarker) -> Retained<Self> {
        let this = Self::alloc(mtm).set_ivars(());
        unsafe { msg_send![super(this), init] }
    }
}

/// Hands `event` to the main menu, as AppKit would have. Raw `msg_send!`: the
/// typed `NSMenu` would need another AppKit feature.
fn replay(mtm: MainThreadMarker, event: &NSEvent) {
    let app = NSApplication::sharedApplication(mtm);
    unsafe {
        let menu: *mut AnyObject = msg_send![&*app, mainMenu];
        if !menu.is_null() {
            let _: bool = msg_send![menu, performKeyEquivalent: event];
        }
    }
}

/// Adds the handler to `page`'s user content controller, and turns off
/// Backspace-goes-back (`backspace.rs`).
pub(crate) fn install(page: &Webview<tauri::Wry>) {
    page.with_webview(|platform| unsafe {
        let ptr = platform.inner() as *mut WKWebView;
        let Some(mtm) = MainThreadMarker::new() else {
            return;
        };
        if ptr.is_null() {
            return;
        }
        super::backspace::disable(&*ptr);
        let controller = (*ptr).configuration().userContentController();
        let handler = KeyFallback::new(mtm);
        controller.addScriptMessageHandler_name(
            ProtocolObject::from_ref(&*handler),
            &NSString::from_str(HANDLER),
        );
    })
    .ok();
}
