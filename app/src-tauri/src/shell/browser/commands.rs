use std::time::Instant;

use tauri::AppHandle;

use super::discard::restore;
use super::layout::{close_tab, hide_all, layout_tab, page, parse};
use super::page::{find_string, go_history, reload_page, ZOOM_MAX, ZOOM_MIN};
use super::seed::wants_sessions;
use super::{broadcast, open_tab, seed_sessions, snapshot, with_state, Snapshot, Viewport};

#[tauri::command]
pub async fn browser_open_url(app: AppHandle, url: String) -> Result<u32, String> {
    open_tab(&app, parse(&url)?)
}

/// For the frontend's mount, which may have missed earlier events.
#[tauri::command]
pub fn browser_state(app: AppHandle) -> Snapshot {
    snapshot(&app)
}

/// A slot for tab `id` has mounted: place and show that page only. A
/// discarded tab's page is made again, and shows once it exists. A closed
/// tab's slot can mount late; its page may be kept for a download and must
/// stay hidden.
#[tauri::command]
pub fn browser_place(app: AppHandle, id: u32, viewport: Viewport) {
    let open = with_state(&app, |s| {
        if !s.tabs.iter().any(|t| t.id == id) {
            return false;
        }
        s.viewports.insert(id, viewport);
        s.show(id);
        true
    });
    if !open {
        return;
    }
    restore(&app, id);
    layout_tab(&app, id);
    if let Some(page) = page(&app, id) {
        page.show().ok();
        page.set_focus().ok();
    }
}

/// Placement only — a hidden page stays hidden, a closed tab's untouched.
#[tauri::command]
pub fn browser_set_viewport(app: AppHandle, id: u32, viewport: Viewport) {
    let open = with_state(&app, |s| {
        let open = s.tabs.iter().any(|t| t.id == id);
        if open {
            s.viewports.insert(id, viewport);
        }
        open
    });
    if open {
        layout_tab(&app, id);
    }
}

/// Gives the app's own webview the keyboard, for a dialog over a page. On
/// Windows a DOM focus cannot move it off a WebView2 page (the same call
/// as `shell/menu/windows.rs`); elsewhere the dialog's own focus does.
#[tauri::command]
pub fn browser_focus_app(app: AppHandle) {
    #[cfg(target_os = "windows")]
    if let Some(main) = tauri::Manager::get_webview(&app, super::MAIN) {
        if let Err(e) = main.set_focus() {
            elog!("[oculus] browser: could not focus the app: {e}");
        }
    }
    #[cfg(not(target_os = "windows"))]
    let _ = app;
}

/// Hidden, not destroyed until it has stayed hidden a while (`discard`).
#[tauri::command]
pub fn browser_hide_tab(app: AppHandle, id: u32) {
    with_state(&app, |s| s.hide(id, Instant::now()));
    if let Some(page) = page(&app, id) {
        page.hide().ok();
    }
}

#[tauri::command]
pub fn browser_hide(app: AppHandle) {
    hide_all(&app);
}

/// Always navigates, even to the same URL.
#[tauri::command]
pub fn browser_navigate(app: AppHandle, id: u32, url: String) -> Result<(), String> {
    let target = parse(&url)?;
    let Some(webview) = page(&app, id) else {
        // A discarded tab loads it when it is shown again.
        let kept = with_state(&app, |s| {
            let discarded = s.discarded.contains(&id);
            match s.tabs.iter_mut().find(|t| t.id == id) {
                Some(tab) if discarded => {
                    tab.url = target.to_string();
                    true
                }
                _ => false,
            }
        });
        if !kept {
            return Err("no such tab".into());
        }
        broadcast(&app);
        return Ok(());
    };
    if wants_sessions(&target) {
        seed_sessions(&app, move || {
            webview.navigate(target).ok();
        });
        return Ok(());
    }
    webview.navigate(target).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn browser_history(app: AppHandle, id: u32, delta: i32) {
    go_history(&app, id, delta);
}

/// `hard` uses WKWebView's `reloadFromOrigin`, which revalidates; a plain
/// reload can keep serving a cached response.
#[tauri::command]
pub fn browser_reload(app: AppHandle, id: u32, hard: bool) {
    let url = with_state(&app, |s| {
        s.tabs.iter().find(|t| t.id == id).map(|t| t.url.clone())
    });
    if url
        .and_then(|u| u.parse().ok())
        .is_some_and(|u| wants_sessions(&u))
    {
        let reload_app = app.clone();
        seed_sessions(&app, move || reload_page(&reload_app, id, hard));
        return;
    }
    reload_page(&app, id, hard);
}

/// The page's own zoom (`pageZoom`), not the app's window zoom.
#[tauri::command]
pub fn browser_set_zoom(app: AppHandle, id: u32, zoom: f64) {
    let zoom = if zoom.is_finite() {
        zoom.clamp(ZOOM_MIN, ZOOM_MAX)
    } else {
        1.0
    };
    let Some(webview) = page(&app, id) else {
        return;
    };
    webview.set_zoom(zoom).ok();
    with_state(&app, |s| {
        if let Some(tab) = s.tabs.iter_mut().find(|t| t.id == id) {
            tab.zoom = zoom;
        }
    });
    broadcast(&app);
}

/// The answer comes back as a `browser-find` event.
#[tauri::command]
pub fn browser_find(app: AppHandle, id: u32, query: String, backwards: bool) {
    if query.is_empty() {
        browser_find_clear(app, id);
        return;
    }
    find_string(&app, id, query, backwards);
}

/// WebKit's find leaves an ordinary selection, so clearing it is the whole job.
#[tauri::command]
pub fn browser_find_clear(app: AppHandle, id: u32) {
    if let Some(webview) = page(&app, id) {
        webview
            .eval("try { window.getSelection().removeAllRanges(); } catch {}")
            .ok();
    }
}

/// The page is stopped and closed for good (`teardown`), so it plays,
/// runs and holds memory no longer — once its downloads end (`close_tab`).
#[tauri::command]
pub fn browser_close_tab(app: AppHandle, id: u32) {
    close_tab(&app, id);
}
