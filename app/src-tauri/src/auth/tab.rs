//! A sign-in made by hand in an in-app browser tab.

use std::sync::atomic::{AtomicBool, Ordering};

use tauri::AppHandle;

use super::{session, session_established, Via};
use crate::credentials::Credentialed;

static CONFIRMING: AtomicBool = AtomicBool::new(false);

/// A browser tab reached a signed-in Canvas page and its cookies are stored
/// in oculus-keyd. If the app is not connected, someone signed in by hand
/// there: once Canvas accepts the session, connect the app as the login
/// window would. Blocks on keyd and Canvas, so not from the main thread.
pub(super) fn confirm_browser_sign_in(app: &AppHandle, keyd: &Credentialed) {
    if session::signed_in(keyd) || CONFIRMING.swap(true, Ordering::SeqCst) {
        return;
    }
    let dir = crate::paths::data_dir();
    match crate::canvas::Canvas::open(&dir).whoami() {
        Ok(name) => {
            eprintln!("[oculus] signed in from a browser tab as {name}");
            session_established(app, &dir, Via::Browser);
        }
        Err(e) => {
            eprintln!("[oculus] browser tab looked signed in, but Canvas refused the session: {e}")
        }
    }
    CONFIRMING.store(false, Ordering::SeqCst);
}
