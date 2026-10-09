//! Canvas HTTP access for the app and the `oculus` CLI: session cookie,
//! retries, Link-header pagination. In Rust, not a WebView — see
//! `docs/architecture.md`.

mod client;
mod cookies;

#[cfg(test)]
mod tests;

use std::path::PathBuf;
use std::sync::Mutex;

pub use cookies::merged_cookie_header;

pub use crate::library::paths::CANVAS_BASE;

/// A finished response. 4xx/5xx are values (a locked module is a 403 to
/// skip); `Err` means the request never completed, after retries.
pub struct Res {
    pub status: u16,
    pub body: Vec<u8>,
    pub content_type: String,
    /// `rel="next"` target from the Link header, if any.
    pub next: Option<String>,
}

impl Res {
    pub fn ok(&self) -> bool {
        (200..300).contains(&self.status)
    }
    pub fn json(&self) -> Result<serde_json::Value, String> {
        serde_json::from_slice(&self.body).map_err(|e| format!("bad JSON: {e}"))
    }
}

/// Returned by [`Canvas::download_to`] when its `cancelled` poll said stop.
pub const CANCELLED: &str = "cancelled";

pub struct Canvas {
    cookie: Mutex<String>,
    cookie_path: PathBuf,
}

/// Outcome of pinging Canvas with the saved session.
pub enum SessionProbe {
    Valid(String),
    Rejected(String),
    Unreachable(String),
}
