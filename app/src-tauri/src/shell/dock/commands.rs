use tauri::AppHandle;

use super::layout::{park, place, webview};
use super::services::service;
use super::{broadcast, focus, page, snapshot, with_state, DockSnapshot, InboxItem};
use crate::shell::browser::Viewport;

/// For the frontend's mount, which may have missed earlier events.
#[tauri::command]
pub fn dock_list(app: AppHandle) -> DockSnapshot {
    snapshot(&app)
}

/// Idempotent: the frontend calls it at boot for every enabled service, and
/// owns the enabled set itself.
#[tauri::command]
pub fn dock_set_enabled(app: AppHandle, id: String, enabled: bool) -> Result<DockSnapshot, String> {
    let service = service(&id).ok_or_else(|| format!("unknown dock service {id}"))?;
    let exists = with_state(&app, |s| s.pages.iter().any(|p| p.id == service.id));
    if enabled && !exists {
        page::create(&app, service)?;
        broadcast(&app);
    } else if !enabled {
        page::close(&app, service.id);
        broadcast(&app);
    }
    Ok(snapshot(&app))
}

/// A slot for `id` has mounted: place that page in it and park whichever
/// page was showing. Never takes keyboard focus; a click in the page does.
#[tauri::command]
pub fn dock_place(app: AppHandle, id: String, viewport: Viewport) {
    let Some(service) = service(&id) else {
        return;
    };
    let previous = with_state(&app, |s| {
        s.viewports.insert(id.clone(), viewport);
        s.last_viewport = Some(viewport);
        s.shown.replace(id.clone())
    });
    if let Some(other) = previous.as_deref().filter(|other| *other != id) {
        if let Some(view) = webview(&app, other) {
            focus::release(&view);
        }
        park(&app, other);
        page::parked(&app, other);
    }
    place(&app, service.id);
    if let Some(view) = webview(&app, service.id) {
        view.show().ok();
    }
    if previous.as_deref() != Some(id.as_str()) {
        broadcast(&app);
    }
}

/// Off-screen at the slot's size, never hidden, so the page keeps running.
#[tauri::command]
pub fn dock_park(app: AppHandle, id: String) {
    if service(&id).is_none() {
        return;
    }
    if let Some(view) = webview(&app, &id) {
        focus::release(&view);
    }
    let was_shown = with_state(&app, |s| {
        let was = s.shown.as_deref() == Some(id.as_str());
        if was {
            s.shown = None;
        }
        was
    });
    park(&app, &id);
    if was_shown {
        page::parked(&app, &id);
        broadcast(&app);
    }
}

/// The slot moved or resized. Only the shown page is placed; a parked one
/// is resized off-screen to match.
#[tauri::command]
pub fn dock_set_viewport(app: AppHandle, id: String, viewport: Viewport) {
    if service(&id).is_none() {
        return;
    }
    let shown = with_state(&app, |s| {
        s.viewports.insert(id.clone(), viewport);
        s.last_viewport = Some(viewport);
        s.shown.as_deref() == Some(id.as_str())
    });
    if shown {
        place(&app, &id);
    } else {
        park(&app, &id);
    }
}

#[tauri::command]
pub fn dock_reload(app: AppHandle, id: String) {
    if let Some(view) = webview(&app, &id) {
        view.reload().ok();
    }
}

/// The Inbox, newest first. In memory only; empty after a relaunch.
#[tauri::command]
pub fn dock_inbox(app: AppHandle) -> Vec<InboxItem> {
    with_state(&app, |s| s.inbox.items())
}

/// Empties the Inbox. Emits nothing: the caller already knows.
#[tauri::command]
pub fn dock_inbox_clear(app: AppHandle) {
    with_state(&app, |s| s.inbox.clear());
}
