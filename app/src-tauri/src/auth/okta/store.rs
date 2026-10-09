//! The username, password and TOTP seed, kept in the macOS keychain.

use super::http::LoginError;
use super::totp::base32_decode;

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

/// Never logged, never written outside the keychain, never sent anywhere but
/// `sso.unimelb.edu.au`.
pub struct Credentials {
    pub username: String,
    pub password: String,
    pub totp_secret: String,
}

impl Credentials {
    /// `Ok(None)` when any piece is missing; a refused keychain read is
    /// `UnreadableCredentials`, never "not set up".
    pub fn load() -> Result<Option<Credentials>, LoginError> {
        let get = |account| read(account).map_err(LoginError::UnreadableCredentials);
        let Some(username) = get("username")? else {
            return Ok(None);
        };
        let Some(password) = get("password")? else {
            return Ok(None);
        };
        let Some(totp_secret) = get("totp_secret")? else {
            return Ok(None);
        };
        Ok(Some(Credentials {
            username,
            password,
            totp_secret,
        }))
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
