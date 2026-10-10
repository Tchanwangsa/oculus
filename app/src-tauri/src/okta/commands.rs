//! The Tauri commands the Settings page calls.

use super::login::run_sign_in;
use super::{clear_credentials, credential_status, store_credentials, CredentialStatus, Trigger};

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
    store_credentials(&username, &password, &totp_secret)
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
