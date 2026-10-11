use tauri::AppHandle;

use super::layout::{hide_all, layout_tab, page, parse};
use super::page::{find_string, go_history, reload_page, ZOOM_MAX, ZOOM_MIN};
use super::seed::wants_sessions;
use super::still::snapshot_page;
use super::{broadcast, open_tab, seed_sessions, snapshot, with_state, Snapshot, Viewport};

#[tauri::command]
pub fn browser_open_url(app: AppHandle, url: String) -> Result<u32, String> {
    open_tab(&app, parse(&url)?)
}

/// For the frontend's mount, which may have missed earlier events.
#[tauri::command]
pub fn browser_state(app: AppHandle) -> Snapshot {
    snapshot(&app)
}

/// A slot for tab `id` has mounted: place and show that page only.
#[tauri::command]
pub fn browser_place(app: AppHandle, id: u32, viewport: Viewport) {
    with_state(&app, |s| {
        s.viewports.insert(id, viewport);
        s.shown.insert(id);
    });
    layout_tab(&app, id);
    if let Some(page) = page(&app, id) {
        page.show().ok();
        page.set_focus().ok();
    }
}

/// Placement only — a hidden page stays hidden.
#[tauri::command]
pub fn browser_set_viewport(app: AppHandle, id: u32, viewport: Viewport) {
    with_state(&app, |s| {
        s.viewports.insert(id, viewport);
    });
    layout_tab(&app, id);
}

/// Hidden, not destroyed.
#[tauri::command]
pub fn browser_hide_tab(app: AppHandle, id: u32) {
    with_state(&app, |s| s.shown.remove(&id));
    if let Some(page) = page(&app, id) {
        page.hide().ok();
    }
}

/// PNG bytes (raw, not base64 — a 2x still is megabytes). On error the
/// caller hides the page instead.
#[tauri::command]
pub async fn browser_snapshot(app: AppHandle, id: u32) -> Result<tauri::ipc::Response, String> {
    let (tx, rx) = tokio::sync::oneshot::channel();
    snapshot_page(&app, id, tx);
    match rx.await {
        Ok(Some(png)) => Ok(tauri::ipc::Response::new(png)),
        _ => Err("no snapshot".into()),
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
    let webview = page(&app, id).ok_or("no such tab")?;
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

/// **`close()` alone does not stop the page.** wry's macOS `Drop` leaks the
/// WKWebView (`removeFromSuperview` + `retain`), which keeps playing media
/// unreachably. So tear the document down while this handle still exists.
#[tauri::command]
pub fn browser_close_tab(app: AppHandle, id: u32) {
    if let Some(webview) = page(&app, id) {
        // Pause lands immediately; `about:blank` commits later, on the
        // leaked view, and destroys the media elements.
        webview
            .eval(
                "for (const m of document.querySelectorAll('video,audio')) \
                 { try { m.pause(); m.removeAttribute('src'); m.load(); } catch {} }",
            )
            .ok();
        if let Ok(blank) = "about:blank".parse() {
            webview.navigate(blank).ok();
        }
        webview.close().ok();
    }
    with_state(&app, |s| {
        s.tabs.retain(|t| t.id != id);
        s.viewports.remove(&id);
        s.shown.remove(&id);
    });
    broadcast(&app);
}
