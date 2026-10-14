use tauri::webview::{NewWindowResponse, PageLoadEvent, Webview, WebviewBuilder};
use tauri::{AppHandle, LogicalPosition, LogicalSize, Manager, WebviewUrl, Window, WindowEvent};

use super::favicon::ensure_favicon;
use super::page::refresh_nav;
use super::seed::{is_sso_app_entry, recover_sso, wants_sessions};
#[cfg(not(target_os = "windows"))]
use super::PAGE_USER_AGENT;
use super::{broadcast, label, seed_sessions, with_state, Tab, Viewport, LABEL_PREFIX, MAIN};
use crate::shell::native_layer::{adopt_page, refresh_page};

/// Seeds the cookie jar and hooks the main window's resize. From `setup`.
pub fn init(app: &AppHandle) {
    #[cfg(target_os = "windows")]
    if let Some(main) = app.get_webview(MAIN) {
        crate::shell::menu::windows::install(&main, false);
    }
    seed_sessions(app, || {});
    let Some(window) = app.get_window(MAIN) else {
        elog!("[oculus] browser: no main window at setup; pages will not follow resizes");
        return;
    };
    let events_app = app.clone();
    window.on_window_event(move |event| {
        if matches!(
            event,
            WindowEvent::Resized(_) | WindowEvent::ScaleFactorChanged { .. }
        ) {
            layout(&events_app);
        }
    });
}

pub(super) fn parse(url: &str) -> Result<url::Url, String> {
    let parsed: url::Url = url.parse().map_err(|e| format!("bad url {url}: {e}"))?;
    match parsed.scheme() {
        // `file://` would expose the disk; custom schemes are the app's own.
        "http" | "https" => Ok(parsed),
        other => Err(format!("refusing to open {other}: scheme")),
    }
}

pub(super) fn page(app: &AppHandle, id: u32) -> Option<Webview<tauri::Wry>> {
    app.get_webview(&label(id))
}

fn tab_id(label: &str) -> Option<u32> {
    label.strip_prefix(LABEL_PREFIX)?.parse().ok()
}

fn pages(app: &AppHandle) -> Vec<Webview<tauri::Wry>> {
    app.webviews()
        .into_iter()
        .filter(|(label, _)| label.starts_with(LABEL_PREFIX))
        .map(|(_, webview)| webview)
        .collect()
}

/// The window's content area in logical points.
pub(crate) fn content_size(window: &Window) -> LogicalSize<f64> {
    let scale = window.scale_factor().unwrap_or(1.0);
    window
        .inner_size()
        .map(|s| s.to_logical::<f64>(scale))
        .unwrap_or_else(|_| LogicalSize::new(1480.0, 920.0))
}

/// One page's rect: the content area minus its insets. An unplaced (hidden)
/// page takes the whole window.
pub(crate) fn rect(
    window: LogicalSize<f64>,
    viewport: Option<Viewport>,
) -> (LogicalPosition<f64>, LogicalSize<f64>) {
    let Some(vp) = viewport else {
        return (LogicalPosition::new(0.0, 0.0), window);
    };
    let width = (window.width - vp.left - vp.right).max(1.0);
    let height = (window.height - vp.top - vp.bottom).max(1.0);
    (
        LogicalPosition::new(vp.left, vp.top),
        LogicalSize::new(width, height),
    )
}

/// The resize handler: puts every placed page back in its slot. Unplaced
/// pages are hidden and skipped.
fn layout(app: &AppHandle) {
    let Some(window) = app.get_window(MAIN) else {
        return;
    };
    let bounds = content_size(&window);
    let viewports = with_state(app, |s| s.viewports.clone());
    for page in pages(app) {
        let Some(vp) = tab_id(page.label()).and_then(|id| viewports.get(&id).copied()) else {
            continue;
        };
        let (position, size) = rect(bounds, Some(vp));
        page.set_position(position).ok();
        page.set_size(size).ok();
        refresh_page(&page);
    }
}

pub(super) fn layout_tab(app: &AppHandle, id: u32) {
    let (Some(window), Some(page)) = (app.get_window(MAIN), page(app, id)) else {
        return;
    };
    let viewport = with_state(app, |s| s.viewports.get(&id).copied());
    let (position, size) = rect(content_size(&window), viewport);
    page.set_position(position).ok();
    page.set_size(size).ok();
    refresh_page(&page);
}

/// Attaches a page webview to a tab. It starts hidden until the frontend
/// places it.
fn create_page(app: &AppHandle, id: u32, url: url::Url) -> Result<(), String> {
    let window = app.get_window(MAIN).ok_or("no main window")?;

    let load_app = app.clone();
    let title_app = app.clone();
    let popup_app = app.clone();
    let builder = WebviewBuilder::new(label(id), WebviewUrl::External(url))
        // A tab opened in the background must not take the keyboard; a shown
        // one is focused below.
        .focused(false)
        // A child webview reports `outerWidth`/`outerHeight` as 0, and canvas
        // renderers (Google Docs) read `outerWidth / innerWidth` as zoom and
        // drop to minimum resolution. Report the viewport's size instead.
        .initialization_script(
            r#"(function(){try{var d=function(k,s){Object.defineProperty(window,k,{configurable:true,get:function(){return window[s];}});};d('outerWidth','innerWidth');d('outerHeight','innerHeight');}catch(e){}})()"#,
        )
        // Page-load events (main frame only), not `on_navigation`, which
        // fires for every iframe — Canvas's LTI shims would hijack the URL.
        .on_page_load(move |webview, payload| {
            let started = matches!(payload.event(), PageLoadEvent::Started);
            let url = payload.url().to_string();
            with_state(&load_app, |s| {
                if let Some(tab) = s.tabs.iter_mut().find(|t| t.id == id) {
                    tab.url = url;
                    tab.loading = started;
                }
            });
            broadcast(&load_app);
            // Both edges: asking twice covers a page that redirects on arrival.
            refresh_nav(&load_app, id);
            if !started {
                // A signed-in Canvas load rolls the session forward;
                // re-snapshot it. A signed-out one would save the anonymous
                // cookie over the good one. If the app is not connected,
                // someone signed in here, so connect it.
                if crate::auth::is_authenticated_url(payload.url()) {
                    crate::auth::save_browser_session(webview.app_handle());
                }
                if is_sso_app_entry(payload.url()) {
                    recover_sso(&load_app, id);
                }
                ensure_favicon(&load_app, payload.url());
            }
        })
        .on_document_title_changed(move |_, title| {
            with_state(&title_app, |s| {
                if let Some(tab) = s.tabs.iter_mut().find(|t| t.id == id) {
                    tab.title = title;
                }
            });
            broadcast(&title_app);
        })
        // Windows creation must leave the WebView2 callback and event loop.
        // WebKit needs the main thread, queued behind its delegate callback
        // through a helper thread so `run_on_main_thread` cannot run inline.
        .on_new_window(move |url, _| {
            let app = popup_app.clone();
            std::thread::spawn(move || {
                let open_app = app.clone();
                let open = move || {
                    if let Err(e) = open_tab(&open_app, url) {
                        elog!("[oculus] browser: could not open popup: {e}");
                    }
                };
                #[cfg(target_os = "windows")]
                open();
                #[cfg(not(target_os = "windows"))]
                app.run_on_main_thread(open).ok();
            });
            NewWindowResponse::Deny
        });

    #[cfg(not(target_os = "windows"))]
    let builder = builder.user_agent(PAGE_USER_AGENT);
    // ⌘-keys the page leaves go to the menu (`shell/keys/fallback.rs`).
    #[cfg(target_os = "macos")]
    let builder = builder.initialization_script_for_all_frames(crate::shell::keys::FALLBACK_SCRIPT);
    let viewport = with_state(app, |s| s.viewports.get(&id).copied());
    let (position, size) = rect(content_size(&window), viewport);
    let webview = window
        .add_child(builder, position, size)
        .map_err(|e| format!("failed to open page webview: {e}"))?;
    #[cfg(target_os = "windows")]
    crate::shell::menu::windows::install(&webview, true);
    adopt_page(&webview);
    crate::shell::native_layer::apply_page_theme(&webview);
    #[cfg(target_os = "macos")]
    crate::shell::keys::install_fallback(&webview);
    if with_state(app, |s| s.shown.contains(&id)) {
        webview.show().ok();
        webview.set_focus().ok();
    } else {
        webview.hide().ok();
    }
    Ok(())
}

pub(super) fn hide_all(app: &AppHandle) {
    with_state(app, |s| s.shown.clear());
    for page in pages(app) {
        page.hide().ok();
    }
}

/// Opens `url` in a new tab — the entry point for every link in the app.
pub fn open_tab(app: &AppHandle, url: url::Url) -> Result<u32, String> {
    let id = with_state(app, |s| {
        s.next_id += 1;
        s.tabs.push(Tab::new(s.next_id, url.to_string()));
        s.next_id
    });
    elog!("[oculus] browser: opening tab {id} → {url}");
    if wants_sessions(&url) {
        broadcast(app);
        let app = app.clone();
        seed_sessions(&app.clone(), move || {
            // Closed while the jar was being seeded.
            if !with_state(&app, |s| s.tabs.iter().any(|t| t.id == id)) {
                return;
            }
            if let Err(e) = create_page(&app, id, url) {
                elog!("[oculus] browser: tab {id}: {e}");
                with_state(&app, |s| s.tabs.retain(|t| t.id != id));
                broadcast(&app);
            }
        });
        return Ok(id);
    }
    if let Err(e) = create_page(app, id, url) {
        with_state(app, |s| s.tabs.retain(|t| t.id != id));
        return Err(e);
    }
    broadcast(app);
    Ok(id)
}
