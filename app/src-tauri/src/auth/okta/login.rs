//! The headless sign-in as the app asks for it.

use keyd_core::okta::{Env, NoSessions};

use super::keychain::Keychain;
use super::{LoginError, Trigger};
use crate::providers::credentials::{Credentialed, KeydError};

/// Headless sign-in behind the attempt guard. keyd runs it, saves the session
/// in its vault and takes the caller's role from the connection. With keyd
/// absent there is no sign-in: a session only keyd can use must not be minted
/// without it.
pub fn sign_in(data_dir: &std::path::Path, trigger: Trigger) -> Result<(), LoginError> {
    sign_in_in(&Credentialed::at(data_dir), trigger)
}

pub(super) fn sign_in_in(broker: &Credentialed, trigger: Trigger) -> Result<(), LoginError> {
    match broker.ensure_signed_in(trigger) {
        Ok(outcome) => outcome,
        Err(KeydError::Absent) => Err(LoginError::Broker(
            "oculus-keyd is not running or not installed, and the Canvas session is kept and \
             used only through it"
                .to_string(),
        )),
        Err(KeydError::Keychain(e)) => Err(LoginError::UnreadableCredentials(e)),
        Err(e) => Err(LoginError::Broker(e.to_string())),
    }
}

/// Clears the attempt guard's failures, pause and wait after a person signed
/// in themselves. keyd does it, so the app never writes the record; only an
/// absent keyd lets this process reset it. A failure is logged: the sign-in
/// itself succeeded.
pub fn resume_automatic_sign_in(data_dir: &std::path::Path) {
    resume_in(&Credentialed::at(data_dir), || {
        keyd_core::okta::resume_automatic_sign_in(data_dir)
    });
}

pub(super) fn resume_in(broker: &Credentialed, in_process: impl FnOnce() -> Result<(), String>) {
    let outcome = match broker.okta_resume() {
        Err(KeydError::Absent) => in_process(),
        other => other.map_err(|e| e.to_string()),
    };
    if let Err(why) = outcome {
        eprintln!("[oculus] the attempt guard was not cleared: {why}");
    }
}

/// What the sign-in page looks like from here, for when the flow fails. It
/// only reads the page, so it keeps no session.
pub fn diagnose() -> String {
    let env = Env::new(
        &crate::library::paths::data_dir(),
        crate::library::paths::CANVAS_BASE,
        &Keychain,
        &NoSessions,
    );
    keyd_core::okta::diagnose(&env)
}

pub(super) fn run_sign_in(
    app: &tauri::AppHandle,
    dir: &std::path::Path,
    trigger: Trigger,
) -> Result<String, String> {
    sign_in(dir, trigger).map_err(|e| e.to_string())?;
    signed_in(app, dir)
}

/// End the headless sign-in the way every sign-in ends, returning the account
/// name.
fn signed_in(app: &tauri::AppHandle, dir: &std::path::Path) -> Result<String, String> {
    crate::auth::session_established(app, dir, crate::auth::Via::Headless);
    crate::sources::canvas::Canvas::open(dir).whoami()
}

/// Called when a probe finds the session dead: rebuild it silently if
/// automated sign-in is set up. `false` means ask the user; every reason but
/// "never set up" and "signed out" is logged, a keychain refusal included.
pub fn try_auto_recover(app: &tauri::AppHandle, trigger: Trigger) -> bool {
    let dir = crate::library::paths::data_dir();
    let outcome = match sign_in(&dir, trigger) {
        Err(LoginError::NotConfigured | LoginError::SignedOut) => return false,
        Err(e) => Err(e.to_string()),
        Ok(()) => signed_in(app, &dir),
    };
    match outcome {
        Ok(name) => {
            eprintln!("[oculus] session rebuilt without a browser ({name})");
            true
        }
        Err(e) => {
            eprintln!("[oculus] automated re-sign-in failed: {e}");
            false
        }
    }
}
