//! Ending a page for good, shared by closing a tab and discarding one,
//! keeping a closed tab's page quietly until its downloads end, and asking
//! whether a page is in use before a discard.
//!
//! macOS: the page stops loading, its media pauses and leaves fullscreen and
//! picture-in-picture, and WebKit's `_close` closes the page in the WebContent
//! process at once rather than whenever the WKWebView deallocates. Then
//! `close()` drops wry's view, the last retain now that `wk::with_view`
//! balances tauri's. Windows: dropping wry's view calls the controller's
//! `Close()`, which ends the page itself.

use tauri::webview::Webview;

/// Stops `webview`'s page and closes it.
pub(super) fn destroy(webview: &Webview<tauri::Wry>) {
    #[cfg(target_os = "macos")]
    if let Err(e) = crate::shell::wk::with_view(webview, |view| unsafe { stop(view) }) {
        elog!(
            "[oculus] browser: {}: could not stop the page: {e}",
            webview.label()
        );
    }
    if let Err(e) = webview.close() {
        elog!(
            "[oculus] browser: {}: could not close: {e}",
            webview.label()
        );
    }
}

/// Hides a closed tab's page and silences it, but keeps it loaded: closing
/// or stopping it could end the download it is kept for.
pub(super) fn park(webview: &Webview<tauri::Wry>) {
    webview.hide().ok();
    #[cfg(target_os = "macos")]
    if let Err(e) = crate::shell::wk::with_view(webview, |view| unsafe { silence(view) }) {
        elog!(
            "[oculus] browser: {}: could not silence the page: {e}",
            webview.label()
        );
    }
}

/// SAFETY: main thread, a live view.
#[cfg(target_os = "macos")]
unsafe fn silence(view: &objc2_web_kit::WKWebView) {
    use objc2::runtime::NSObjectProtocol;
    use objc2::sel;

    // Both are macOS 12 and later.
    if view.respondsToSelector(sel!(pauseAllMediaPlaybackWithCompletionHandler:)) {
        view.pauseAllMediaPlaybackWithCompletionHandler(None);
    }
    if view.respondsToSelector(sel!(closeAllMediaPresentationsWithCompletionHandler:)) {
        view.closeAllMediaPresentationsWithCompletionHandler(None);
    }
}

/// SAFETY: main thread, a live view.
#[cfg(target_os = "macos")]
unsafe fn stop(view: &objc2_web_kit::WKWebView) {
    use objc2::runtime::NSObjectProtocol;
    use objc2::{msg_send, sel};

    view.stopLoading();
    silence(view);
    // WebKit SPI (`WKWebViewPrivate.h`), which Safari calls on every tab it
    // closes.
    if view.respondsToSelector(sel!(_close)) {
        let _: () = msg_send![view, _close];
    }
}

/// Answers whether `webview`'s page plays media or captures the camera or
/// microphone, so discarding it would cut something off. Unanswerable counts
/// as in use. `reply` may never run if the page goes first.
#[cfg(target_os = "macos")]
pub(super) fn in_use(webview: &Webview<tauri::Wry>, reply: impl FnOnce(bool) + Send + 'static) {
    use std::cell::Cell;

    use block2::RcBlock;
    use objc2::runtime::NSObjectProtocol;
    use objc2::sel;
    use objc2_web_kit::{WKMediaCaptureState, WKMediaPlaybackState};

    let queued = crate::shell::wk::with_view(webview, move |view| unsafe {
        let capturing = view.respondsToSelector(sel!(cameraCaptureState))
            && (view.cameraCaptureState() != WKMediaCaptureState::None
                || view.microphoneCaptureState() != WKMediaCaptureState::None);
        let askable =
            view.respondsToSelector(sel!(requestMediaPlaybackStateWithCompletionHandler:));
        if capturing || !askable {
            reply(true);
            return;
        }
        let reply = Cell::new(Some(reply));
        let answer = RcBlock::new(move |state: WKMediaPlaybackState| {
            if let Some(reply) = reply.take() {
                reply(state == WKMediaPlaybackState::Playing);
            }
        });
        view.requestMediaPlaybackStateWithCompletionHandler(&answer);
    });
    if let Err(e) = queued {
        elog!("[oculus] browser: {}: {e}", webview.label());
    }
}

/// WebView2 reports audio only (`IsDocumentPlayingAudio`); it has no capture
/// state, and a call plays audio.
#[cfg(target_os = "windows")]
pub(super) fn in_use(webview: &Webview<tauri::Wry>, reply: impl FnOnce(bool) + Send + 'static) {
    use webview2_com::Microsoft::Web::WebView2::Win32::ICoreWebView2_8;
    use windows_core::{Interface, BOOL};

    let queued = webview.with_webview(move |platform| {
        let playing = unsafe {
            platform
                .controller()
                .CoreWebView2()
                .and_then(|core| core.cast::<ICoreWebView2_8>())
                .and_then(|core| {
                    let mut playing = BOOL::default();
                    core.IsDocumentPlayingAudio(&mut playing)?;
                    Ok(playing.as_bool())
                })
        };
        reply(playing.unwrap_or(true));
    });
    if let Err(e) = queued {
        elog!("[oculus] browser: {}: {e}", webview.label());
    }
}

/// Nothing to ask; never discarded.
#[cfg(not(any(target_os = "macos", target_os = "windows")))]
pub(super) fn in_use(_webview: &Webview<tauri::Wry>, reply: impl FnOnce(bool) + Send + 'static) {
    reply(true);
}
