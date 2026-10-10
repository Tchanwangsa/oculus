//! The keychain items the credentials fall back to while keyd is absent.

use keyd_core::okta::{validate_credentials, CredentialStore, Credentials};

use super::credentials::CredentialStatus;
use super::LoginError;

const KEYCHAIN_SERVICE: &str = "com.oculus.unimelb-sso";

fn secret(account: &str) -> crate::providers::credentials::Secret<'_> {
    crate::providers::credentials::Secret::new(KEYCHAIN_SERVICE, account)
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
pub(super) struct Keychain;

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

pub(super) fn keychain_status() -> Result<CredentialStatus, String> {
    let unreadable = |e| LoginError::UnreadableCredentials(e).to_string();
    Ok(CredentialStatus {
        username: read("username").map_err(unreadable)?,
        has_password: read("password").map_err(unreadable)?.is_some(),
        has_totp: read("totp_secret").map_err(unreadable)?.is_some(),
    })
}

/// Validates with `keyd_core::okta::validate_credentials`, the check keyd
/// applies too, then saves all three. A new save lifts the attempt guard.
pub(super) fn keychain_store(
    username: &str,
    password: &str,
    totp_secret: &str,
) -> Result<(), String> {
    let creds = validate_credentials(username, password, totp_secret)?;
    write("username", &creds.username)?;
    write("password", &creds.password)?;
    write("totp_secret", &creds.totp_secret)?;
    if let Err(why) = keyd_core::okta::resume_automatic_sign_in(&crate::library::paths::data_dir())
    {
        eprintln!("[oculus] the attempt guard was not cleared: {why}");
    }
    Ok(())
}

pub(super) fn keychain_forget() -> Result<(), String> {
    erase("username")?;
    erase("password")?;
    erase("totp_secret")?;
    Ok(())
}

/// Drop only the password, keeping username and seed — the response to
/// `LoginError::BadPassword`.
fn clear_password() -> Result<(), String> {
    erase("password")
}
