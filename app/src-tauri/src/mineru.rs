//! MinerU cloud credential storage.
//!
//! With `oculus-keyd` installed the token lives in its vault, and these
//! commands go through keyd; without it, in the macOS keychain
//! (`credentials::CloudKey`). Never in SQLite or the WebView.
//! `parse::mineru::client` asks keyd for each request, or reads the keychain
//! at construction when keyd is absent.

use std::time::Duration;

use crate::credentials::{CloudKey, Secret, Verdict};

/// The name keyd stores the token under.
pub(crate) const SECRET: &str = "mineru";

pub(crate) static KEY: CloudKey = CloudKey {
    secret: SECRET,
    what: "MinerU token",
    keychain: Secret::new("com.tchan.oculus.mineru", "mineru"),
};

/// A task id that cannot exist: a good token answers "no such task", a bad
/// one 401. Nothing is created or charged.
const PROBE_URL: &str =
    "https://mineru.net/api/v4/extract/task/00000000-0000-0000-0000-000000000000";

/// `Err` when the keychain refused, as opposed to holding no token.
pub(crate) fn fetch_api_key() -> Result<Option<String>, String> {
    KEY.fetch()
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
/// and the token was stored on trust. The probe runs here with the token
/// just typed; only the store goes through keyd.
#[tauri::command]
pub fn mineru_set_api_key(key: String) -> Result<String, String> {
    KEY.set(&key, probe)
}

#[tauri::command]
pub fn mineru_has_api_key() -> Result<bool, String> {
    KEY.has()
}

#[tauri::command]
pub fn mineru_delete_api_key() -> Result<(), String> {
    KEY.delete()
}
