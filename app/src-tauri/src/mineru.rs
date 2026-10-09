//! MinerU cloud credential storage.
//!
//! The token lives only in the macOS keychain, never in SQLite or the
//! WebView; `parse::mineru::client` reads it at construction.

use std::time::Duration;

use crate::credentials::{Secret, Verdict};

const KEY: Secret = Secret::new("com.tchan.oculus.mineru", "mineru");

/// A task id that cannot exist: a good token answers "no such task", a bad
/// one 401. Nothing is created or charged.
const PROBE_URL: &str =
    "https://mineru.net/api/v4/extract/task/00000000-0000-0000-0000-000000000000";

pub(crate) fn stored_api_key() -> Option<String> {
    KEY.read()
}

/// Ask MinerU whether it accepts this token. `Err` is a token MinerU actively
/// refused, and carries the message the settings page shows.
fn probe(key: &str) -> Result<Verdict, String> {
    match ureq::get(PROBE_URL)
        .timeout(Duration::from_secs(10))
        .set("Authorization", &format!("Bearer {key}"))
        .set("Accept", "application/json")
        .call()
    {
        // Anything but an auth failure got past the gateway.
        Ok(_) => Ok(Verdict::Good),
        Err(ureq::Error::Status(401 | 403, response)) => {
            let code = response
                .into_string()
                .ok()
                .and_then(|body| serde_json::from_str::<serde_json::Value>(&body).ok())
                .and_then(|v| {
                    v.get("msgCode")
                        .or_else(|| v.get("code"))
                        .and_then(|c| c.as_str())
                        .map(str::to_string)
                });
            Err(match code.as_deref() {
                Some("A0211") => "MinerU says this token has expired (A0211) — \
                     create a new one on its API Management page"
                    .to_string(),
                Some(code) => {
                    format!("MinerU rejected this token ({code}) — check you copied all of it")
                }
                None => "MinerU rejected this token — check you copied all of it".to_string(),
            })
        }
        Err(ureq::Error::Status(_, _)) => Ok(Verdict::Good),
        Err(_) => Ok(Verdict::Unverified),
    }
}

/// Store a token, but only one MinerU has agreed to. Returns `"ok"` when it
/// was checked against MinerU and `"unverified"` when MinerU was unreachable
/// and the token was stored on trust.
#[tauri::command]
pub fn mineru_set_api_key(key: String) -> Result<String, String> {
    KEY.store_checked(&key, probe)
}

#[tauri::command]
pub fn mineru_has_api_key() -> Result<bool, String> {
    KEY.has("MinerU token")
}

#[tauri::command]
pub fn mineru_delete_api_key() -> Result<(), String> {
    KEY.delete()
}

