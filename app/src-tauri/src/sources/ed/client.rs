//! The session half of `Ed`: authenticated GETs through oculus-keyd, token
//! renewal, and saving a pasted token.

use super::{Ed, ED_API, ED_BASE, TIMEOUT};

use std::path::Path;
use std::sync::Mutex;

use keyd_core::client::{SessionKind, SESSION_TIMEOUT};

use crate::providers::credentials::{Credentialed, KeydError};

/// Why an Ed request has no answer, in words for the log and the CLI.
pub(super) fn keyd_failure(error: KeydError) -> String {
    match error {
        KeydError::Absent => "oculus-keyd is not running or not installed, and Ed is reached \
                              only through it (`oculus keyd status`)."
            .to_string(),
        KeydError::Missing(_) | KeydError::NoSession(..) => {
            "No saved Ed token — run `oculus auth ed <TOKEN>`, or sync a course with an Ed \
             Discussion tool."
                .to_string()
        }
        other => other.to_string(),
    }
}

impl Ed {
    /// A client of the oculus-keyd serving `data_dir`.
    pub fn open(data_dir: &Path) -> Self {
        Ed {
            keyd: Credentialed::at(data_dir),
            courses: Mutex::new(None),
        }
    }

    /// Whether oculus-keyd holds an Ed token right now.
    pub fn has_session(&self) -> bool {
        self.keyd.session_status().is_ok_and(|s| s.ed)
    }

    /// Validate a pasted token against `/api/user`, hand it to oculus-keyd,
    /// return the name. The check is a direct request: the token is not
    /// stored yet, and a bad paste must not replace a working session.
    pub fn set_token(data_dir: &Path, token: &str) -> Result<String, String> {
        store_token(&Credentialed::at(data_dir), ED_BASE, token)
    }

    pub fn whoami(&self) -> Result<String, String> {
        let user = self.get("/user")?;
        Ok(user["user"]["name"]
            .as_str()
            .unwrap_or("Ed user")
            .to_string())
    }

    /// GET `/api{path}` with oculus-keyd attaching the token.
    pub(super) fn get(&self, path: &str) -> Result<serde_json::Value, String> {
        let reply = self
            .keyd
            .send(
                "ed",
                "GET",
                &format!("{ED_API}{path}"),
                &[],
                b"",
                SESSION_TIMEOUT,
            )
            .map_err(keyd_failure)?;
        json_reply(reply.status, &reply.body, path)
    }

    /// Extend the session and keep the fresh token. Best-effort.
    pub(super) fn renew(&self) {
        let Ok(reply) = self.keyd.send(
            "ed",
            "POST",
            &format!("{ED_API}/renew_token"),
            &[],
            b"",
            SESSION_TIMEOUT,
        ) else {
            return;
        };
        if !(200..300).contains(&reply.status) {
            return;
        }
        // The token is in the body, so this is the one place it reaches us.
        let Some(new) = serde_json::from_slice::<serde_json::Value>(&reply.body)
            .ok()
            .and_then(|v| v["token"].as_str().map(str::to_string))
            .filter(|t| !t.is_empty())
        else {
            return;
        };
        if let Err(e) = self.keyd.session_put(SessionKind::Ed, &new) {
            eprintln!("[oculus] ed token was not saved: {e}");
        }
    }
}

/// An Ed API answer as JSON.
fn json_reply(status: u16, body: &[u8], path: &str) -> Result<serde_json::Value, String> {
    if (200..300).contains(&status) {
        return serde_json::from_slice(body).map_err(|e| format!("bad JSON: {e}"));
    }
    Err(status_error(status, path))
}

fn status_error(status: u16, path: &str) -> String {
    match status {
        401 => "Ed rejected the token (401) — run `oculus auth ed <TOKEN>` with a fresh one"
            .to_string(),
        code => format!("HTTP {code} for {path}"),
    }
}

/// Check `token` against `base`/user directly, then give it to oculus-keyd.
pub(super) fn store_token(keyd: &Credentialed, base: &str, token: &str) -> Result<String, String> {
    let token = token.trim();
    if token.is_empty() {
        return Err("The Ed token is empty.".to_string());
    }
    let url = format!("{base}/user");
    let user: serde_json::Value = match ureq::get(&url)
        .timeout(TIMEOUT)
        .set("x-token", token)
        .call()
    {
        Ok(r) => r
            .into_string()
            .map_err(|e| format!("unreadable response: {e}"))
            .and_then(|s| serde_json::from_str(&s).map_err(|e| format!("bad JSON: {e}")))?,
        Err(ureq::Error::Status(code, _)) => return Err(status_error(code, "/user")),
        Err(e) => return Err(e.to_string()),
    };
    let name = user["user"]["name"]
        .as_str()
        .unwrap_or("Ed user")
        .to_string();
    keyd.session_put(SessionKind::Ed, token)
        .map_err(keyd_failure)?;
    Ok(name)
}
