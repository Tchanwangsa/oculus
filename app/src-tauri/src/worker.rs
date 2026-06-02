use tauri::{AppHandle, Manager, WebviewUrl, WebviewWindowBuilder};

pub const WORKER_LABEL: &str = "canvas-worker";

/// Hidden WebView that hosts the scraper / subjects JS. It loads a blank page
/// served from the local IPC origin, so its fetches to the cookie proxy
/// (`/canvas`) and result callbacks (`/scrape`, `/subjects`) are same-origin —
/// no CORS, and it needs no Canvas login of its own. All Canvas auth happens
/// server-side in the proxy via the persisted session cookie.
pub fn ensure_worker_window(app: &AppHandle, port: u16) {
    if app.get_webview_window(WORKER_LABEL).is_some() {
        return;
    }
    let url = format!("http://127.0.0.1:{port}/worker");
    match WebviewWindowBuilder::new(
        app,
        WORKER_LABEL,
        WebviewUrl::External(url.parse().unwrap()),
    )
    .title("Oculus worker")
    .inner_size(800.0, 600.0)
    .visible(false)
    .skip_taskbar(true)
    .build()
    {
        Ok(_) => eprintln!("[oculus] worker window ready on :{port}/worker"),
        Err(e) => eprintln!("[oculus] worker window failed: {e}"),
    }
}
