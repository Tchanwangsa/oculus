//! Creating and closing a service's page, and setting up its data store.

use tauri::webview::{NewWindowResponse, PageLoadEvent, Webview, WebviewBuilder};
use tauri::{AppHandle, Manager, WebviewUrl};

use super::layout::{parked_rect, place, slot};
use super::scripts::init_scripts;
use super::services::DockService;
use super::{broadcast, channel, focus, label, title, with_state, DockPageInfo};
use crate::shell::browser::{content_size, MAIN, PAGE_USER_AGENT};

/// Opens the service's page parked, or in its slot if the frontend already
/// placed it. Called only when the service is enabled and has no page.
pub(super) fn create(app: &AppHandle, service: &'static DockService) -> Result<(), String> {
    let window = app.get_window(MAIN).ok_or("no main window")?;
    let id = service.id;
    let home: url::Url = service
        .home
        .parse()
        .map_err(|e| format!("bad dock url {}: {e}", service.home))?;

    let load_app = app.clone();
    let title_app = app.clone();
    let mut builder = WebviewBuilder::new(label(id), WebviewUrl::External(home.clone()))
        .user_agent(PAGE_USER_AGENT)
        .data_store_identifier(service.store_id)
        .focused(false)
        // Main frame only; `on_navigation` would also see every iframe.
        .on_page_load(move |_, payload| {
            let loading = matches!(payload.event(), PageLoadEvent::Started);
            let url = payload.url().to_string();
            with_state(&load_app, |s| {
                if let Some(page) = s.pages.iter_mut().find(|p| p.id == id) {
                    page.url = url;
                    page.loading = loading;
                }
            });
            broadcast(&load_app);
        })
        .on_document_title_changed(move |_, text| {
            with_state(&title_app, |s| {
                if let Some(page) = s.pages.iter_mut().find(|p| p.id == id) {
                    page.unread = title::unread(&text);
                    page.title = text;
                }
            });
            broadcast(&title_app);
        })
        // A dock page has no tabs, so `target=_blank` and `window.open` go to
        // the default browser.
        .on_new_window(|url, _| {
            if matches!(url.scheme(), "http" | "https") {
                if let Err(e) = tauri_plugin_opener::open_url(url.as_str(), None::<&str>) {
                    elog!("[oculus] dock: could not open {url}: {e}");
                }
            }
            NewWindowResponse::Deny
        });
    // Main frame, document start.
    for script in init_scripts(service) {
        builder = builder.initialization_script(script);
    }

    let viewport = with_state(app, |s| {
        s.pages.push(DockPageInfo {
            id: id.to_string(),
            title: String::new(),
            url: home.to_string(),
            loading: true,
            unread: 0,
        });
        slot(s, id)
    });
    let (position, size) = parked_rect(content_size(&window), viewport);
    let webview = match window.add_child(builder, position, size) {
        Ok(webview) => webview,
        Err(e) => {
            with_state(app, |s| s.pages.retain(|p| p.id != id));
            return Err(format!("failed to open dock page {id}: {e}"));
        }
    };
    configure_store(&webview, service);
    channel::install(&webview, app, service);
    if with_state(app, |s| s.shown.as_deref() == Some(id)) {
        place(app, id);
    }
    Ok(())
}

/// wry's macOS `Drop` leaks the WKWebView, which would keep the page's
/// sockets and media alive unreachably, so it is emptied before `close()`.
pub(super) fn close(app: &AppHandle, id: &str) {
    if let Some(webview) = app.get_webview(&label(id)) {
        focus::release(&webview);
        channel::remove(&webview);
        if let Ok(blank) = "about:blank".parse() {
            webview.navigate(blank).ok();
        }
        webview.close().ok();
    }
    with_state(app, |s| {
        s.pages.retain(|p| p.id != id);
        s.viewports.remove(id);
        if s.shown.as_deref() == Some(id) {
            s.shown = None;
        }
    });
}

/// Logs which store the page got and exempts the service's sites from ITP.
/// `data_store_identifier` needs macOS 14; below that wry silently uses the
/// shared default store, which is left untouched. `_setPersistedSites:` is
/// private SPI from macOS 15.2: without it WebKit wipes a site's script
/// storage (the WhatsApp and Discord logins) 30 operating days after the
/// last click or key press in it.
#[cfg(target_os = "macos")]
fn configure_store(page: &Webview<tauri::Wry>, service: &'static DockService) {
    use objc2::rc::Retained;
    use objc2::{msg_send, sel};
    use objc2_foundation::{NSArray, NSString, NSURL};
    use objc2_web_kit::WKWebView;

    use super::services::uuid_string;

    page.with_webview(move |platform| unsafe {
        let ptr = platform.inner() as *mut WKWebView;
        if ptr.is_null() {
            return;
        }
        let store = (*ptr).configuration().websiteDataStore();
        let wanted = uuid_string(&service.store_id);
        match store.identifier().map(|u| u.UUIDString().to_string()) {
            Some(got) if got == wanted => {
                elog!("[oculus] dock: {} uses store {got}", service.id);
            }
            got => {
                elog!(
                    "[oculus] dock: {} got store {} instead of {wanted} \
                     (per-service stores need macOS 14); its login is shared",
                    service.id,
                    got.as_deref().unwrap_or("default"),
                );
                return;
            }
        }
        let responds: bool = msg_send![&*store, respondsToSelector: sel!(_setPersistedSites:)];
        if !responds {
            elog!(
                "[oculus] dock: {}: no _setPersistedSites: (macOS 15.2+); \
                 ITP may sign it out after 30 days unused",
                service.id
            );
            return;
        }
        let sites: Vec<Retained<NSURL>> = service
            .sites
            .iter()
            .filter_map(|site| NSURL::URLWithString(&NSString::from_str(site)))
            .collect();
        let sites = NSArray::from_retained_slice(&sites);
        let _: () = msg_send![&*store, _setPersistedSites: &*sites];
    })
    .ok();
}

#[cfg(not(target_os = "macos"))]
fn configure_store(_page: &Webview<tauri::Wry>, _service: &'static DockService) {}
