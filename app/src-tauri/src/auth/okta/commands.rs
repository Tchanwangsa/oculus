//! Tauri commands for the Okta credentials and the automatic recovery path.

use super::attempts::{resume_automatic_sign_in, sign_in, Trigger};
use super::http::LoginError;
use super::store::{clear_credentials, credential_status, store_credentials, CredentialStatus};

#[tauri::command]
pub fn okta_credential_status() -> Result<CredentialStatus, String> {
    credential_status()
}

#[tauri::command]
pub fn okta_save_credentials(
    username: String,
    password: String,
    totp_secret: String,
) -> Result<(), String> {
    store_credentials(&username, &password, &totp_secret)?;
    resume_automatic_sign_in(&crate::library::paths::data_dir());
    Ok(())
}

#[tauri::command]
pub fn okta_clear_credentials() -> Result<(), String> {
    clear_credentials()
}

#[tauri::command]
pub async fn okta_sign_in(app: tauri::AppHandle) -> Result<String, String> {
    crate::runtime::blocking::run(move || {
        run_sign_in(&app, &crate::library::paths::data_dir(), Trigger::Manual)
    })
    .await
}

fn run_sign_in(
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
    // The headless path works on this account, so a re-authenticating
    // LaunchAgent is worth installing.
    crate::auth::keepalive::ensure_installed();
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
        Ok(_) => signed_in(app, &dir),
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
