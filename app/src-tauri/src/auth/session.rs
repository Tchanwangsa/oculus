//! Where every sign-in ends: the auth flag, in-memory state and UI event.

use super::{auth_flag_path, AuthProbe, AuthState};
use std::sync::atomic::{AtomicBool, Ordering};
use tauri::{AppHandle, Emitter, Manager};

/// Pings the Canvas API with the saved session cookie. Doubles as the
/// keep-alive: the client writes back the rotated cookie.
pub fn saved_session_probe() -> AuthProbe {
    let probe = crate::sources::canvas::Canvas::open(&crate::library::paths::data_dir()).probe();

    match &probe {
        AuthProbe::Valid(name) => eprintln!("[oculus] session check: valid ({name})"),
        AuthProbe::Rejected(why) => eprintln!("[oculus] session check: rejected — {why}"),
        AuthProbe::Unreachable(why) => eprintln!("[oculus] session check: inconclusive — {why}"),
    }
    probe
}

/// How a sign-in happened, for the one place every sign-in in the app ends.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Via {
    Window,
    Browser,
    Headless,
}

/// Every sign-in in the app ends here once its cookie is on disk: the auth
/// flag (which also lifts a sign-out), the in-memory state and the UI event.
/// A person's sign-in also clears the attempt guard's wait and pause; a
/// headless one has already settled the guard.
pub fn session_established(app: &AppHandle, dir: &std::path::Path, via: Via) {
    crate::library::paths::mark_authenticated(dir);
    if via != Via::Headless {
        crate::auth::okta::resume_automatic_sign_in(dir);
    }
    if let Some(state) = app.try_state::<AuthState>() {
        *state.0.lock().unwrap() = true;
    }
    app.emit("canvas-auth-success", "ok").ok();
}

static CONFIRMING: AtomicBool = AtomicBool::new(false);

/// A browser tab reached a signed-in Canvas page while the app is not
/// connected: someone signed in by hand there. Its snapshot is already saved;
/// once Canvas accepts it, connect the app as the login window would.
pub fn confirm_browser_sign_in(app: &AppHandle) {
    if auth_flag_path().exists() || CONFIRMING.swap(true, Ordering::SeqCst) {
        return;
    }
    let app = app.clone();
    std::thread::spawn(move || {
        let dir = crate::library::paths::data_dir();
        match crate::sources::canvas::Canvas::open(&dir).whoami() {
            Ok(name) => {
                eprintln!("[oculus] signed in from a browser tab as {name}");
                session_established(&app, &dir, Via::Browser);
            }
            Err(e) => eprintln!(
                "[oculus] browser tab looked signed in, but Canvas refused the session: {e}"
            ),
        }
        CONFIRMING.store(false, Ordering::SeqCst);
    });
}
