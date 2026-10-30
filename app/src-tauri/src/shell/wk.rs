//! `with_webview` without the leak. On macOS tauri-runtime-wry hands the
//! closure the WKWebView, its user content controller and its window as raw
//! pointers made with `Retained::into_raw`, so every call left one retain of
//! each behind. A browser page's WKWebView then never deallocated after
//! `close()`, and its WebContent process kept running. `with_view` takes the
//! three retains back once the closure returns; every `with_webview` on macOS
//! goes through it.

use objc2::rc::Retained;
use objc2::runtime::AnyObject;
use objc2_web_kit::WKWebView;
use tauri::webview::{PlatformWebview, Webview};
use tauri::Runtime;

/// The retains one `with_webview` call handed over, released on drop.
struct Handed {
    view: Option<Retained<WKWebView>>,
    _manager: Option<Retained<AnyObject>>,
    _window: Option<Retained<AnyObject>>,
}

impl Handed {
    /// SAFETY: once per `PlatformWebview`, which owns one retain of each.
    unsafe fn take(platform: &PlatformWebview) -> Self {
        Self {
            view: Retained::from_raw(platform.inner().cast()),
            _manager: Retained::from_raw(platform.controller().cast()),
            _window: Retained::from_raw(platform.ns_window().cast()),
        }
    }
}

/// Runs `f` on the main thread with `webview`'s WKWebView, then releases what
/// `with_webview` retained. `f` does not run if there is no view.
pub(crate) fn with_view<R: Runtime>(
    webview: &Webview<R>,
    f: impl FnOnce(&WKWebView) + Send + 'static,
) -> tauri::Result<()> {
    webview.with_webview(move |platform| {
        let handed = unsafe { Handed::take(&platform) };
        if let Some(view) = handed.view.as_deref() {
            f(view);
        }
    })
}
