//! Windows: pages stay above the opaque app webview, each clipped by a
//! window region on its wry container (the HWND
//! `ICoreWebView2Controller::ParentWindow` names). The region is the open
//! holes over the page minus every portal covering them, so a popover draws
//! over the live page, and a page with no hole over it is hidden. A
//! transparent WebView2 does not show a sibling under it, so the macOS
//! layering cannot work here.
//!
//! Every Win32 call runs inside `with_webview`, on the event loop that owns
//! these windows, and reads the container's rect there, so a region always
//! matches where the page is when it applies.

use std::collections::HashMap;
use std::sync::Mutex;

use tauri::webview::Webview;
use tauri::{AppHandle, Manager};
use windows_sys::Win32::Foundation::{HWND, POINT, RECT};
use windows_sys::Win32::Graphics::Gdi::{MapWindowPoints, SetWindowRgn};
use windows_sys::Win32::UI::HiDpi::GetDpiForWindow;
use windows_sys::Win32::UI::WindowsAndMessaging::{
    GetParent, GetWindowRect, SetWindowPos, HWND_DESKTOP, HWND_TOP, SWP_NOACTIVATE, SWP_NOMOVE,
    SWP_NOSIZE,
};

use super::gdi;
use super::region::{page_region, Cover, Hole, PageRegion, Px};
use crate::shell::browser;

/// The frontend's latest holes and covers, and the region each container
/// last got, so an unchanged one is not set (and redrawn) again.
#[derive(Default)]
pub struct NativeHoles {
    inner: Mutex<Inner>,
}

#[derive(Default)]
struct Inner {
    holes: Vec<Hole>,
    covers: Vec<Cover>,
    /// Keyed by the container's HWND.
    applied: HashMap<usize, PageRegion>,
}

/// From `setup`, before any page exists.
pub fn init(app: &AppHandle) {
    app.manage(NativeHoles::default());
}

/// The holes open over pages and the portals over them, in logical window
/// points. Sent whole on every change; an empty `holes` hides every page.
#[tauri::command]
pub fn native_set_holes(app: AppHandle, holes: Vec<Hole>, covers: Vec<Cover>) {
    if let Some(state) = app.try_state::<NativeHoles>() {
        let mut inner = state.inner.lock().unwrap();
        inner.holes = holes;
        inner.covers = covers;
    }
    for page in pages(&app) {
        refresh_page(&page);
    }
}

/// Raises a just-added page above the app and gives it the current region,
/// empty until the frontend opens a hole over it. Called right after
/// `add_child`.
pub fn adopt_page(webview: &Webview<tauri::Wry>) {
    with_container(webview, |app, container| {
        // SAFETY: a live child window of this thread; z-order only.
        unsafe {
            SetWindowPos(
                container,
                HWND_TOP,
                0,
                0,
                0,
                0,
                SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
            );
        }
        if let Some(state) = app.try_state::<NativeHoles>() {
            // A new window may reuse a closed one's handle.
            let mut inner = state.inner.lock().unwrap();
            inner.applied.remove(&(container as usize));
        }
        apply(app, container);
    });
}

/// Re-applies a page's region after Rust moved or resized it.
pub fn refresh_page(webview: &Webview<tauri::Wry>) {
    with_container(webview, apply);
}

/// Every browser tab in the main window.
fn pages(app: &AppHandle) -> Vec<Webview<tauri::Wry>> {
    app.webviews()
        .into_iter()
        .filter(|(label, _)| label.starts_with(browser::LABEL_PREFIX))
        .map(|(_, webview)| webview)
        .collect()
}

/// Runs `f` on the event loop with the page's container HWND.
fn with_container(
    webview: &Webview<tauri::Wry>,
    f: impl FnOnce(&AppHandle, HWND) + Send + 'static,
) {
    let app = webview.app_handle().clone();
    let label = webview.label().to_string();
    let queued = webview.with_webview(move |platform| {
        let mut container: HWND = std::ptr::null_mut();
        // SAFETY: an out-pointer to one HWND, which is layout-compatible
        // with the `windows` crate's newtype the binding names.
        let got = unsafe {
            platform
                .controller()
                .ParentWindow(&mut container as *mut _ as _)
        };
        if got.is_err() || container.is_null() {
            elog!("[oculus] native layer: {label}: no container window");
            return;
        }
        f(&app, container);
    });
    if let Err(e) = queued {
        elog!("[oculus] native layer: {e}");
    }
}

/// Computes and sets one container's region.
fn apply(app: &AppHandle, container: HWND) {
    let Some(state) = app.try_state::<NativeHoles>() else {
        return;
    };
    let Some(frame) = frame(container) else {
        return;
    };
    // SAFETY: a live window; 0 means the call failed.
    let dpi = unsafe { GetDpiForWindow(container) };
    let scale = if dpi == 0 { 1.0 } else { dpi as f64 / 96.0 };
    let mut inner = state.inner.lock().unwrap();
    let region = page_region(&inner.holes, &inner.covers, frame, scale);
    if inner.applied.get(&(container as usize)) == Some(&region) {
        return;
    }
    let Some(rgn) = gdi::build(&region) else {
        elog!("[oculus] native layer: could not build a page region");
        return;
    };
    // SAFETY: a live window and region; redraws at once.
    if unsafe { SetWindowRgn(container, rgn.handle(), 1) } == 0 {
        elog!("[oculus] native layer: SetWindowRgn failed");
        return;
    }
    rgn.give_away();
    inner.applied.insert(container as usize, region);
}

/// The container's rect in its parent's client area (the main window's,
/// where the frontend measures), physical pixels.
fn frame(container: HWND) -> Option<Px> {
    let mut rect = RECT {
        left: 0,
        top: 0,
        right: 0,
        bottom: 0,
    };
    // SAFETY: live windows and a two-point buffer, which is what a RECT is.
    unsafe {
        if GetWindowRect(container, &mut rect) == 0 {
            return None;
        }
        let parent = GetParent(container);
        if parent.is_null() {
            return None;
        }
        MapWindowPoints(
            HWND_DESKTOP,
            parent,
            &mut rect as *mut RECT as *mut POINT,
            2,
        );
    }
    Some(Px {
        left: rect.left,
        top: rect.top,
        right: rect.right,
        bottom: rect.bottom,
    })
}
