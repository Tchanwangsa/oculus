//! Backspace never navigates back.
//!
//! WebKit's `BackspaceKeyNavigationEnabled` defaults to on in a `WKWebView`
//! (Safari turns it off), so a Backspace the page doesn't cancel goes back a
//! page — on a circuit or diagram editor that listens without cancelling, a
//! delete becomes a navigation. Back stays on the toolbar button and ⌘[.

use objc2::msg_send;
use objc2::runtime::AnyObject;
use objc2_web_kit::WKWebView;

/// Turns the preference off on `view`'s live `WKPreferences`. Private API,
/// so it is skipped where WebKit no longer answers to it.
pub(crate) fn disable(view: &WKWebView) {
    unsafe {
        let prefs: *mut AnyObject = msg_send![&*view.configuration(), preferences];
        if prefs.is_null() {
            return;
        }
        let setter = objc2::sel!(_setBackspaceKeyNavigationEnabled:);
        let responds: bool = msg_send![prefs, respondsToSelector: setter];
        if responds {
            let _: () = msg_send![prefs, _setBackspaceKeyNavigationEnabled: false];
        }
    }
}
