//! The visible Canvas login window, for a sign-in that needs a person. It
//! opens at the profile's login path, which hands off to whatever IdP the
//! university uses.

mod confirmation;

use std::sync::{Arc, Mutex};

use tauri::webview::PageLoadEvent;
use tauri::{AppHandle, Emitter, Manager, WebviewUrl, WebviewWindowBuilder};

use super::{
    announce_sign_in, cookies, is_authenticated_url, record_sign_in, saved_session_probe,
    AuthProbe, Via,
};
use confirmation::Confirmation;

fn canvas_session_dir() -> std::path::PathBuf {
    crate::library::paths::data_dir().join("canvas-session")
}

/// Opens the login window. On success it hands the webview's cookies to
/// oculus-keyd, marks the app signed in, then hides itself.
pub fn open_canvas_window(app: AppHandle) -> Result<(), String> {
    let inst = crate::sources::institution::current()
        .ok_or_else(|| crate::sources::institution::NOT_CONFIGURED.to_string())?;
    let url: url::Url = format!("{}{}", inst.canvas, inst.login_path)
        .parse()
        .map_err(|e| format!("bad Canvas address: {e}"))?;

    if let Some(existing) = app.get_webview_window("canvas-auth") {
        existing.close().ok();
        std::thread::sleep(std::time::Duration::from_millis(200));
    }

    let session_dir = canvas_session_dir();
    let dir = crate::library::paths::data_dir();
    let app_nav = app.clone();
    let app_win = app.clone();
    let confirmation = Arc::new(Mutex::new(Confirmation::default()));
    let confirmation_load = Arc::clone(&confirmation);

    let result = WebviewWindowBuilder::new(&app, "canvas-auth", WebviewUrl::External(url))
        .title("Sign in to Canvas — Oculus")
        .inner_size(900.0, 700.0)
        .center()
        .visible(true)
        .data_directory(session_dir)
        .on_page_load(move |_, payload| {
            if !matches!(payload.event(), PageLoadEvent::Finished)
                || !is_authenticated_url(payload.url())
                || !confirmation_load.lock().unwrap().request()
            {
                return;
            }
            // The response has set its cookies. Reading the jar and probing
            // keyd must run outside WebKit's page-load delegate callback.
            let app = app_nav.clone();
            let dir = dir.clone();
            let confirmation = Arc::clone(&confirmation_load);
            std::thread::spawn(move || loop {
                if confirmation.lock().unwrap().closed() {
                    return;
                }
                let stored = cookies::save_session_cookie(&app);
                let accepted = stored && matches!(saved_session_probe(), AuthProbe::Valid(_));
                if confirmation.lock().unwrap().closed() {
                    return;
                }
                if accepted && record_sign_in(&app, &dir, Via::Window) {
                    if !confirmation
                        .lock()
                        .unwrap()
                        .resolve(|| announce_sign_in(&app))
                    {
                        return;
                    }
                    if let Some(w) = app.get_webview_window("canvas-auth") {
                        w.hide().ok();
                    }
                    elog!("[oculus] auth success");
                    return;
                }
                elog!("[oculus] auth: the saved Canvas session was not confirmed");
                if confirmation.lock().unwrap().retry() {
                    continue;
                }
                app.emit("canvas-auth-cancelled", "cancelled").ok();
                return;
            });
        })
        .build();

    let win = match result {
        Ok(w) => w,
        Err(e) => {
            elog!("[oculus] failed to open canvas window: {e}");
            return Err(format!("could not open the sign-in window: {e}"));
        }
    };

    win.on_window_event(move |event| {
        if let tauri::WindowEvent::CloseRequested { .. } = event {
            if confirmation.lock().unwrap().close() {
                app_win.emit("canvas-auth-cancelled", "cancelled").ok();
            }
        }
    });
    Ok(())
}
