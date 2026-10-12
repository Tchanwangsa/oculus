//! Session restore at launch.

use tauri::{Emitter, Manager};

use super::{saved_session_probe, session, AuthProbe, AuthState};
use crate::providers::credentials::{Credentialed, KeydError};

/// If oculus-keyd says the app was signed in, replay the stored session with
/// a server-side ping. Rejected → try auto-recover, else clear the flag;
/// unreachable → stay optimistic, since offline is not expired. Runs on its
/// own thread: asking keyd can wait on its keychain prompt.
pub fn restore_session(app: &tauri::AppHandle) {
    let app = app.clone();
    std::thread::spawn(move || {
        let keyd = Credentialed::at(&crate::library::paths::data_dir());
        if !session::signed_in(&keyd) {
            elog!("[oculus] no auth flag — fresh session");
            return;
        }
        // A keyd that has not taken the profile's university gets nothing,
        // so the stored session cannot be used: the UI asks for a sign-in,
        // which reports why.
        if let Err(e @ KeydError::Site(_)) = crate::sources::institution::bound() {
            elog!("[oculus] the stored session stays unused: {e}");
            app.emit("canvas-auth-expired", "expired").ok();
            return;
        }
        elog!("[oculus] auth flag found — verifying persisted session");
        // Optimistic until the check below corrects it.
        let mem = std::sync::Arc::clone(&app.state::<AuthState>().0);
        *mem.lock().unwrap() = true;

        match saved_session_probe() {
            AuthProbe::Valid(_) => {
                app.emit("canvas-auth-success", "ok").ok();
            }
            AuthProbe::Rejected(_) => {
                // `try_auto_recover` emits its own success event.
                if !crate::auth::okta::try_auto_recover(&app, crate::auth::okta::Trigger::Startup) {
                    elog!("[oculus] session rejected — reset to disconnected");
                    session::mark_session_dead(&keyd);
                    *mem.lock().unwrap() = false;
                    app.emit("canvas-auth-expired", "expired").ok();
                }
            }
            AuthProbe::Unreachable(_) => {
                elog!("[oculus] could not verify session — assuming still good");
            }
        }
    });
}
