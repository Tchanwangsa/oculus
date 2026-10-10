//! The app's side of the Canvas sign-in. Sessions live in oculus-keyd's
//! vault; this module is how a person's sign-in reaches it (the login window
//! and in-app browser tabs hand over what their webview holds), how the app
//! learns whether it is signed in, and how it signs out (`docs/auth.md`).

pub(crate) mod commands;
mod session;
mod snapshot;
mod startup;
mod tab;
mod watch;
mod window;

use std::sync::{Arc, Mutex};

use tauri::{AppHandle, Emitter, Manager};

use crate::credentials::Credentialed;

pub use crate::canvas::SessionProbe as AuthProbe;
pub(crate) use session::report;
pub use snapshot::{save_browser_session, save_session_cookie};
pub use startup::restore_session;
pub use watch::{signed_in_since, SignInWatch};

pub struct AuthState(pub Arc<Mutex<bool>>);

/// Drops every session oculus-keyd holds, the authenticated flag, and stands
/// automatic sign-in down. True when there was something to drop.
pub fn sign_out(data_dir: &std::path::Path) -> Result<bool, String> {
    session::sign_out(&Credentialed::at(data_dir))
}

pub fn is_authenticated_url(url: &url::Url) -> bool {
    url.host_str() == Some("canvas.lms.unimelb.edu.au") && {
        let p = url.path();
        // `/?login_success=1` is the success signal: the JS redirect to the
        // dashboard after it fires no nav event.
        p == "/"
            || p.starts_with("/dashboard")
            || p.starts_with("/courses")
            || p.starts_with("/calendar")
            || p.starts_with("/inbox")
    }
}

/// Pings the Canvas API through oculus-keyd, which holds the session and
/// writes back what Canvas rotates.
pub fn saved_session_probe() -> AuthProbe {
    let probe = crate::canvas::Canvas::open(&crate::paths::data_dir()).probe();

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

/// Every sign-in in the app ends here once its session is in oculus-keyd: the
/// authenticated flag (which also lifts a sign-out), the in-memory state and
/// the UI event. A headless sign-in is keyd's own and has marked itself; a
/// person's is marked here, and clears the attempt guard's wait and pause. If
/// keyd cannot record it, the app is not signed in: the UI is told the
/// sign-in did not complete.
pub fn session_established(app: &AppHandle, dir: &std::path::Path, via: Via) {
    if via != Via::Headless {
        if let Err(e) = session::mark_signed_in(&Credentialed::at(dir)) {
            session::report("could not record the sign-in", &e);
            app.emit("canvas-auth-cancelled", "cancelled").ok();
            return;
        }
        crate::okta::resume_automatic_sign_in(dir);
    }
    if let Some(state) = app.try_state::<AuthState>() {
        *state.0.lock().unwrap() = true;
    }
    app.emit("canvas-auth-success", "ok").ok();
}
