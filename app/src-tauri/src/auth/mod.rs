//! Canvas sign-in: cookie snapshots, the login window and the commands the UI
//! drives it with. Okta (headless re-sign-in) and the launchd keep-alive are
//! children.

pub(crate) mod commands;
mod cookies;
pub mod keepalive;
mod login_window;
pub mod okta;
mod session;

use std::sync::{Arc, Mutex};

pub use cookies::{save_session_cookie, saved_cookie_header, saved_sso_cookie_header};
pub use login_window::{is_authenticated_url, open_canvas_window};
pub use session::{confirm_browser_sign_in, saved_session_probe, session_established, Via};

pub use crate::sources::canvas::SessionProbe as AuthProbe;

pub struct AuthState(pub Arc<Mutex<bool>>);

pub fn auth_flag_path() -> std::path::PathBuf {
    crate::library::paths::auth_flag_path(&crate::library::paths::data_dir())
}
