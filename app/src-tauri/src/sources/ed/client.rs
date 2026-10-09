//! The session half of `Ed`: the persisted token, authenticated GETs and
//! token renewal.

use super::{Ed, ED_BASE, TIMEOUT};

use std::path::Path;

use std::sync::Mutex;

impl Ed {
    /// Load the persisted token; without one, [`Ed::has_session`] is false.
    pub fn open(data_dir: &Path) -> Self {
        let token_path = crate::library::paths::ed_token_path(data_dir);
        let token = std::fs::read_to_string(&token_path).unwrap_or_default();
        Ed {
            token: Mutex::new(token.trim().to_string()),
            token_path,
            courses: Mutex::new(None),
        }
    }

    pub fn has_session(&self) -> bool {
        !self.token.lock().unwrap().is_empty()
    }

    /// Validate a pasted token against `/api/user`, persist it, return the name.
    pub fn set_token(data_dir: &Path, token: &str) -> Result<String, String> {
        let token = token.trim();
        let user = get_json(token, "/user")?;
        let name = user["user"]["name"]
            .as_str()
            .unwrap_or("Ed user")
            .to_string();
        crate::library::paths::write_private(
            &crate::library::paths::ed_token_path(data_dir),
            token,
        )
        .map_err(|e| e.to_string())?;
        Ok(name)
    }

    pub fn whoami(&self) -> Result<String, String> {
        if !self.has_session() {
            return Err("No saved Ed token.".to_string());
        }
        let user = self.get("/user")?;
        Ok(user["user"]["name"]
            .as_str()
            .unwrap_or("Ed user")
            .to_string())
    }

    pub(super) fn get(&self, path: &str) -> Result<serde_json::Value, String> {
        let token = self.token.lock().unwrap().clone();
        get_json(&token, path)
    }

    /// Extend the session and persist the fresh token. Best-effort.
    pub(super) fn renew(&self) {
        let token = self.token.lock().unwrap().clone();
        if token.is_empty() {
            return;
        }
        let Ok(resp) = ureq::post(&format!("{ED_BASE}/renew_token"))
            .timeout(TIMEOUT)
            .set("x-token", &token)
            .send_string("")
        else {
            return;
        };
        let Some(new) = resp
            .into_string()
            .ok()
            .and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok())
            .and_then(|v| v["token"].as_str().map(str::to_string))
            .filter(|t| !t.is_empty())
        else {
            return;
        };
        if new != token {
            if let Err(e) = crate::library::paths::write_private(&self.token_path, &new) {
                eprintln!("[oculus] ed token write failed: {e}");
            }
            *self.token.lock().unwrap() = new;
        }
    }
}

fn get_json(token: &str, path: &str) -> Result<serde_json::Value, String> {
    if token.is_empty() {
        return Err("No saved Ed token.".to_string());
    }
    let url = format!("{ED_BASE}{path}");
    let resp = ureq::get(&url)
        .timeout(TIMEOUT)
        .set("x-token", token)
        .call();
    match resp {
        Ok(r) => r
            .into_string()
            .map_err(|e| format!("unreadable response: {e}"))
            .and_then(|s| serde_json::from_str(&s).map_err(|e| format!("bad JSON: {e}"))),
        Err(ureq::Error::Status(401, _)) => Err(
            "Ed rejected the token (401) — run `oculus auth ed <TOKEN>` with a fresh one"
                .to_string(),
        ),
        Err(ureq::Error::Status(code, _)) => Err(format!("HTTP {code} for {path}")),
        Err(e) => Err(e.to_string()),
    }
}
