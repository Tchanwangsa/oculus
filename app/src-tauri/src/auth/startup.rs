//! Session restore at launch.

use tauri::{Emitter, Manager};

use super::{saved_session_probe, session, AuthProbe, AuthState};
use crate::credentials::Credentialed;

/// If oculus-keyd says the app was signed in, replay the stored session with
/// a server-side ping. Rejected → try auto-recover, else clear the flag;
/// unreachable → stay optimistic, since offline is not expired. Runs on its
/// own thread: asking keyd can wait on its keychain prompt.
pub fn restore_session(app: &tauri::AppHandle) {
    let app = app.clone();
    std::thread::spawn(move || {
        let keyd = Credentialed::at(&crate::paths::data_dir());
        if !session::signed_in(&keyd) {
            eprintln!("[oculus] no auth flag — fresh session");
            return;
        }
        eprintln!("[oculus] auth flag found — verifying persisted session");
        // Optimistic until the check below corrects it.
        let mem = std::sync::Arc::clone(&app.state::<AuthState>().0);
        *mem.lock().unwrap() = true;

        match saved_session_probe() {
            AuthProbe::Valid(_) => {
                app.emit("canvas-auth-success", "ok").ok();
            }
            AuthProbe::Rejected(_) => {
                // `try_auto_recover` emits its own success event.
                if !crate::okta::try_auto_recover(&app, crate::okta::Trigger::Startup) {
                    eprintln!("[oculus] session rejected — reset to disconnected");
                    session::mark_session_dead(&keyd);
                    *mem.lock().unwrap() = false;
                    app.emit("canvas-auth-expired", "expired").ok();
                }
            }
            AuthProbe::Unreachable(_) => {
                eprintln!("[oculus] could not verify session — assuming still good");
            }
        }
    });
}
