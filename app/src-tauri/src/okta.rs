//! The app's half of the headless University of Melbourne SSO sign-in.
//!
//! The flow itself (Okta's IDX state machine, the SAML round trip, the
//! attempt guard) is `keyd_core::okta`, which keyd runs too. Here it runs
//! in-process with the credentials in the macOS keychain, and the Tauri
//! commands and the work after a sign-in live here.
//!
//! Password and seed share the keychain, so to anything running as this user
//! the second factor is not a second factor — the same deliberate trade as a
//! password manager holding TOTP.

use keyd_core::okta::{base32_decode, CredentialStore, Credentials, Env};
pub use keyd_core::okta::{resume_automatic_sign_in, totp_now, LoginError, Trigger, SSO_HOST};

// ── Stored credentials ───────────────────────────────────────────────────────

const KEYCHAIN_SERVICE: &str = "com.oculus.unimelb-sso";

fn secret(account: &str) -> crate::credentials::Secret<'_> {
    crate::credentials::Secret::new(KEYCHAIN_SERVICE, account)
}

/// `Err` when the keychain refused the read, as opposed to holding nothing.
fn read(account: &str) -> Result<Option<String>, String> {
    Ok(secret(account).fetch()?.filter(|s| !s.is_empty()))
}

fn write(account: &str, value: &str) -> Result<(), String> {
    secret(account).write(value)
}

fn erase(account: &str) -> Result<(), String> {
    secret(account).delete()
}

/// The keychain, as the sign-in's credential store.
struct Keychain;

impl CredentialStore for Keychain {
    /// `Ok(None)` when any piece is missing; a refused read is `Err`, never
    /// "not set up".
    fn load(&self) -> Result<Option<Credentials>, String> {
        let Some(username) = read("username")? else {
            return Ok(None);
        };
        let Some(password) = read("password")? else {
            return Ok(None);
        };
        let Some(totp_secret) = read("totp_secret")? else {
            return Ok(None);
        };
        Ok(Some(Credentials {
            username,
            password,
            totp_secret,
        }))
    }

    fn clear_password(&self) -> Result<(), String> {
        clear_password()
    }
}

/// Which pieces are on file, for the settings UI; values never leave the
/// keychain.
#[derive(serde::Serialize)]
pub struct CredentialStatus {
    pub username: Option<String>,
    pub has_password: bool,
    pub has_totp: bool,
}

pub fn credential_status() -> Result<CredentialStatus, String> {
    let unreadable = |e| LoginError::UnreadableCredentials(e).to_string();
    Ok(CredentialStatus {
        username: read("username").map_err(unreadable)?,
        has_password: read("password").map_err(unreadable)?.is_some(),
        has_totp: read("totp_secret").map_err(unreadable)?.is_some(),
    })
}

/// Validates the TOTP seed first: an undecodable one would otherwise surface
/// mid sign-in as an indistinguishable "wrong code".
pub fn store_credentials(username: &str, password: &str, totp_secret: &str) -> Result<(), String> {
    let username = username.trim();
    let secret = totp_secret.trim().replace(' ', "");
    if username.is_empty() {
        return Err("Username is required.".to_string());
    }
    if password.is_empty() {
        return Err("Password is required.".to_string());
    }
    base32_decode(&secret).map_err(|e| format!("That does not look like a TOTP setup key: {e}"))?;

    write("username", username)?;
    write("password", password)?;
    write("totp_secret", &secret)?;
    Ok(())
}

/// Forget everything. Called on explicit disconnect, and on a rejected
/// password so a stale secret is not replayed until Okta locks the account.
pub fn clear_credentials() -> Result<(), String> {
    erase("username")?;
    erase("password")?;
    erase("totp_secret")?;
    Ok(())
}

/// Drop only the password, keeping username and seed — the response to
/// `LoginError::BadPassword`.
pub fn clear_password() -> Result<(), String> {
    erase("password")
}

// ── The flow, in this process ────────────────────────────────────────────────

fn env(data_dir: &std::path::Path) -> Env<'static> {
    Env::new(data_dir, crate::paths::CANVAS_BASE, &Keychain)
}

/// Headless sign-in behind the attempt guard (`keyd_core::okta::sign_in`).
pub fn sign_in(data_dir: &std::path::Path, trigger: Trigger) -> Result<String, LoginError> {
    keyd_core::okta::sign_in(&env(data_dir), trigger)
}

/// What the sign-in page looks like from here, for when the flow fails.
pub fn diagnose() -> String {
    keyd_core::okta::diagnose(&env(&crate::paths::data_dir()))
}

// ── Tauri commands ───────────────────────────────────────────────────────────

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
    resume_automatic_sign_in(&crate::paths::data_dir());
    Ok(())
}

#[tauri::command]
pub fn okta_clear_credentials() -> Result<(), String> {
    clear_credentials()
}

#[tauri::command]
pub async fn okta_sign_in(app: tauri::AppHandle) -> Result<String, String> {
    crate::blocking::run(move || run_sign_in(&app, &crate::paths::data_dir(), Trigger::Manual))
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
    crate::keepalive::ensure_installed();
    crate::canvas::Canvas::open(dir).whoami()
}

/// Called when a probe finds the session dead: rebuild it silently if
/// automated sign-in is set up. `false` means ask the user; every reason but
/// "never set up" and "signed out" is logged, a keychain refusal included.
pub fn try_auto_recover(app: &tauri::AppHandle, trigger: Trigger) -> bool {
    let dir = crate::paths::data_dir();
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
