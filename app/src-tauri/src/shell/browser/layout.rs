use tauri::webview::{NewWindowResponse, PageLoadEvent, Webview, WebviewBuilder};
use tauri::{AppHandle, LogicalPosition, LogicalSize, Manager, WebviewUrl, Window, WindowEvent};

use super::favicon::ensure_favicon;
use super::page::refresh_nav;
use super::seed::{is_sso_app_entry, recover_sso, wants_sessions};
use super::{
    broadcast, label, seed_sessions, with_state, Tab, Viewport, LABEL_PREFIX, MAIN, PAGE_USER_AGENT,
};

/// Seeds the cookie jar and hooks the main window's resize. From `setup`.
pub fn init(app: &AppHandle) {
    seed_sessions(app, || {});
    let Some(window) = app.get_window(MAIN) else {
        eprintln!("[oculus] browser: no main window at setup; pages will not follow resizes");
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
fn content_size(window: &Window) -> LogicalSize<f64> {
    let scale = window.scale_factor().unwrap_or(1.0);
    window
        .inner_size()
        .map(|s| s.to_logical::<f64>(scale))
        .unwrap_or_else(|_| LogicalSize::new(1480.0, 920.0))
}

/// One page's rect: the content area minus its insets. An unplaced (hidden)
/// page takes the whole window.
fn rect(
    window: LogicalSize<f64>,
    viewport: Option<Viewport>,
) -> (LogicalPosition<f64>, LogicalSize<f64>, f64) {
    let Some(vp) = viewport else {
        return (LogicalPosition::new(0.0, 0.0), window, 0.0);
    };
    let width = (window.width - vp.left - vp.right).max(1.0);
    let height = (window.height - vp.top - vp.bottom).max(1.0);
    (
        LogicalPosition::new(vp.left, vp.top),
        LogicalSize::new(width, height),
        vp.radius.max(0.0),
    )
}

/// Rounds a page's bottom corners to the card's radius; a native view over
/// the DOM has to clip itself.
#[cfg(target_os = "macos")]
fn round_corners(page: &Webview<tauri::Wry>, radius: f64) {
    use objc2::msg_send;
    use objc2::runtime::AnyObject;

    page.with_webview(move |platform| unsafe {
        let view = platform.inner() as *mut AnyObject;
        if view.is_null() {
            return;
        }
        let _: () = msg_send![view, setWantsLayer: true];
        let layer: *mut AnyObject = msg_send![view, layer];
        if layer.is_null() {
            return;
        }
        // CACornerMask: MinXMaxY | MaxXMaxY. WKWebView is a flipped view, so
        // its layer's MaxY edge is the bottom.
        let mask: usize = (1 << 2) | (1 << 3);
        let _: () = msg_send![layer, setMasksToBounds: true];
        let _: () = msg_send![layer, setCornerRadius: radius];
        let _: () = msg_send![layer, setMaskedCorners: mask];
    })
    .ok();
}

#[cfg(not(target_os = "macos"))]
fn round_corners(_page: &Webview<tauri::Wry>, _radius: f64) {}

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
        let (position, size, radius) = rect(bounds, Some(vp));
        page.set_position(position).ok();
        page.set_size(size).ok();
        round_corners(&page, radius);
    }
}

pub(super) fn layout_tab(app: &AppHandle, id: u32) {
    let (Some(window), Some(page)) = (app.get_window(MAIN), page(app, id)) else {
        return;
    };
    let viewport = with_state(app, |s| s.viewports.get(&id).copied());
    let (position, size, radius) = rect(content_size(&window), viewport);
    page.set_position(position).ok();
    page.set_size(size).ok();
    round_corners(&page, radius);
}

/// Attaches a page webview to a tab. It starts hidden until the frontend
/// places it.
fn create_page(app: &AppHandle, id: u32, url: url::Url) -> Result<(), String> {
    let window = app.get_window(MAIN).ok_or("no main window")?;

    let load_app = app.clone();
    let title_app = app.clone();
    let popup_app = app.clone();
    let builder = WebviewBuilder::new(label(id), WebviewUrl::External(url))
        .user_agent(PAGE_USER_AGENT)
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
                    crate::auth::save_session_cookie(webview.app_handle());
                    crate::auth::confirm_browser_sign_in(webview.app_handle());
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
        // `target=_blank`/`window.open` become a new tab. On the main thread
        // `run_on_main_thread` runs inline, inside WebKit's delegate callback,
        // so hop through a helper thread to queue it behind the callback.
        .on_new_window(move |url, _| {
            let app = popup_app.clone();
            std::thread::spawn(move || {
                app.clone()
                    .run_on_main_thread(move || {
                        open_tab(&app, url).ok();
                    })
                    .ok();
            });
            NewWindowResponse::Deny
        });

    let viewport = with_state(app, |s| s.viewports.get(&id).copied());
    let (position, size, radius) = rect(content_size(&window), viewport);
    let webview = window
        .add_child(builder, position, size)
        .map_err(|e| format!("failed to open page webview: {e}"))?;
    round_corners(&webview, radius);
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
    eprintln!("[oculus] browser: opening tab {id} → {url}");
    if wants_sessions(&url) {
        broadcast(app);
        let app = app.clone();
        seed_sessions(&app.clone(), move || {
            // Closed while the jar was being seeded.
            if !with_state(&app, |s| s.tabs.iter().any(|t| t.id == id)) {
                return;
            }
            if let Err(e) = create_page(&app, id, url) {
                eprintln!("[oculus] browser: tab {id}: {e}");
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
