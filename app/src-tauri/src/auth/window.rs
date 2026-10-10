//! The visible Canvas SAML login window, for a sign-in that needs a person.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use tauri::{AppHandle, Emitter, Manager, WebviewUrl, WebviewWindowBuilder};

use super::{is_authenticated_url, session_established, snapshot, Via};

fn canvas_session_dir() -> std::path::PathBuf {
    crate::paths::data_dir().join("canvas-session")
}

/// Opens the login window. On success it hands the webview's cookies to
/// oculus-keyd, marks the app signed in, then hides itself.
pub fn open_canvas_window(app: AppHandle, auth_flag: Arc<Mutex<bool>>) {
    if let Some(existing) = app.get_webview_window("canvas-auth") {
        existing.close().ok();
        std::thread::sleep(std::time::Duration::from_millis(200));
    }

    let url = "https://canvas.lms.unimelb.edu.au/login/saml";

    let session_dir = canvas_session_dir();
    let dir = crate::paths::data_dir();
    let app_nav = app.clone();
    let app_win = app.clone();
    let auth_flag_nav = Arc::clone(&auth_flag);
    let auth_flag_win = Arc::clone(&auth_flag);

    let resolved = Arc::new(AtomicBool::new(false));
    let resolved_nav = Arc::clone(&resolved);

    let result = WebviewWindowBuilder::new(
        &app,
        "canvas-auth",
        WebviewUrl::External(url.parse().unwrap()),
    )
    .title("Sign in to Canvas — Oculus")
    .inner_size(900.0, 700.0)
    .center()
    .visible(true)
    .data_directory(session_dir)
    .on_navigation(move |url| {
        eprintln!("[oculus] nav: {url}");
        if is_authenticated_url(&url) {
            let was_resolved = resolved_nav.swap(true, Ordering::SeqCst);
            if !was_resolved {
                // Now, so closing the window meanwhile is not a cancel.
                *auth_flag_nav.lock().unwrap() = true;

                // Give Canvas a moment to set the session cookie first.
                let app_delayed = app_nav.clone();
                let dir = dir.clone();
                std::thread::spawn(move || {
                    std::thread::sleep(std::time::Duration::from_secs(2));
                    if !snapshot::save_session_cookie(&app_delayed) {
                        eprintln!("[oculus] auth: the Canvas session was not stored");
                        app_delayed.emit("canvas-auth-cancelled", "cancelled").ok();
                        return;
                    }
                    session_established(&app_delayed, &dir, Via::Window);
                    if let Some(w) = app_delayed.get_webview_window("canvas-auth") {
                        w.hide().ok();
                    }
                    eprintln!("[oculus] auth success");
                });
            }
        }
        true
    })
    .build();

    let win = match result {
        Ok(w) => w,
        Err(e) => {
            eprintln!("[oculus] failed to open canvas window: {e}");
            return;
        }
    };

    win.on_window_event(move |event| {
        if let tauri::WindowEvent::CloseRequested { .. } = event {
            if !*auth_flag_win.lock().unwrap() {
                app_win.emit("canvas-auth-cancelled", "cancelled").ok();
            }
        }
    });
}
