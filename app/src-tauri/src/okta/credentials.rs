//! The saved sign-in credentials, through keyd and (keyd absent) the keychain.

use super::keychain::{keychain_forget, keychain_status, keychain_store};
use super::{broker, LoginError};
use crate::credentials::{Credentialed, KeydError};

/// Which pieces are on file, for the settings UI; values never leave keyd or
/// the keychain.
#[derive(Debug, serde::Serialize)]
pub struct CredentialStatus {
    pub username: Option<String>,
    pub has_password: bool,
    pub has_totp: bool,
}

// Each `*_in` asks `broker` and runs its fallback only when keyd is absent.

pub fn credential_status() -> Result<CredentialStatus, String> {
    credential_status_in(&broker(), keychain_status)
}

pub(super) fn credential_status_in(
    broker: &Credentialed,
    keychain: impl FnOnce() -> Result<CredentialStatus, String>,
) -> Result<CredentialStatus, String> {
    match broker.okta_status() {
        Ok(status) => Ok(CredentialStatus {
            username: status.username,
            has_password: status.has_password,
            has_totp: status.has_totp,
        }),
        Err(KeydError::Absent) => keychain(),
        Err(KeydError::Keychain(e)) => Err(LoginError::UnreadableCredentials(e).to_string()),
        Err(e) => Err(format!(
            "Could not check the saved sign-in credentials: {e}"
        )),
    }
}

/// Saves all three. keyd validates; its message for bad input is passed on
/// as written.
pub fn store_credentials(username: &str, password: &str, totp_secret: &str) -> Result<(), String> {
    store_credentials_in(&broker(), username, password, totp_secret, || {
        keychain_store(username, password, totp_secret)
    })
}

pub(super) fn store_credentials_in(
    broker: &Credentialed,
    username: &str,
    password: &str,
    totp_secret: &str,
    keychain: impl FnOnce() -> Result<(), String>,
) -> Result<(), String> {
    match broker.okta_save(username, password, totp_secret) {
        Err(KeydError::Absent) => keychain(),
        Err(KeydError::Request(message)) => Err(message),
        other => other.map_err(|e| e.to_string()),
    }
}

/// Forget everything. Called on explicit disconnect.
pub fn clear_credentials() -> Result<(), String> {
    clear_credentials_in(&broker(), keychain_forget)
}

pub(super) fn clear_credentials_in(
    broker: &Credentialed,
    keychain: impl FnOnce() -> Result<(), String>,
) -> Result<(), String> {
    match broker.okta_forget() {
        Err(KeydError::Absent) => keychain(),
        other => other.map(|_| ()).map_err(|e| e.to_string()),
    }
}
