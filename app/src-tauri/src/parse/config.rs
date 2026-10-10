//! Which backend parses, read from the `parse` settings row.

use super::{ParseError, Parser};
use serde::{Deserialize, Serialize};

/// Which backend *is* the parser. Only the `engine` key selects one; the
/// blob's legacy `backend` key (`local | cloud | auto`, a fallback policy) is
/// ignored, because its "local" named a different program. Absent means
/// `Cloud`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Engine {
    Cloud,
    Local,
}

impl Engine {
    pub fn as_str(self) -> &'static str {
        match self {
            Engine::Cloud => "cloud",
            Engine::Local => "local",
        }
    }

    /// Where this engine lives when nothing overrides it; also the settings
    /// page's placeholder, so the two cannot differ.
    pub fn default_base_url(self) -> &'static str {
        match self {
            Engine::Cloud => CLOUD_BASE_URL,
            Engine::Local => LOCAL_BASE_URL,
        }
    }

    /// Anything that is not one of the two names — `"auto"` included — is not
    /// an engine, and is treated as if the field were absent.
    pub(super) fn parse(value: &str) -> Option<Self> {
        match value.trim() {
            "cloud" => Some(Engine::Cloud),
            "local" => Some(Engine::Local),
            _ => None,
        }
    }
}

/// Where a backend's token comes from, if it needs one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CredentialSource {
    /// `oculus-keyd` when it is installed and the API root is MinerU's own,
    /// else the macOS keychain via `crate::providers::mineru`
    /// (`MinerUCloud::with_config`). The token never enters SQLite or the
    /// WebView.
    Keychain,
    /// Loopback to a server on this machine: nothing to authenticate.
    None,
}

impl CredentialSource {
    /// The keychain's token, for the path that runs with keyd absent. A
    /// refusal is `UnreadableCredentials`, never "no token".
    pub fn token(self) -> Result<Option<String>, ParseError> {
        match self {
            CredentialSource::Keychain => {
                crate::providers::mineru::fetch_api_key().map_err(ParseError::UnreadableCredentials)
            }
            CredentialSource::None => Ok(None),
        }
    }
}

#[derive(Debug, Clone)]
pub struct ParseConfig {
    pub engine: Engine,
    /// API root for the chosen engine, overridable from the settings blob.
    pub base_url: String,
    pub credentials: CredentialSource,
    /// Download results through the CDN's expired certificate (see
    /// `super::mineru::result_tls`). Defaults to on.
    pub accept_expired_result_cert: bool,
}

/// MinerU's published API root.
pub const CLOUD_BASE_URL: &str = "https://mineru.net/api/v4";

/// The local parse server's default origin: where MinerU's own server binds
/// by default. `engineUrl` overrides it.
pub const LOCAL_BASE_URL: &str = "http://127.0.0.1:8000";

/// The `parse` row, as far as this seam cares. Every field is optional; the
/// blob's other keys are ignored.
#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub(super) struct StoredParseSettings {
    /// A string, not `Engine`: a stale value costs this field, not the row.
    pub(super) engine: Option<String>,
    pub(super) engine_url: Option<String>,
    pub(super) accept_expired_result_cert: Option<bool>,
}

/// The backend selection from the `parse` settings row. Unreadable, missing
/// or nonsense settings resolve to the cloud default.
pub fn parse_config() -> ParseConfig {
    let stored = stored_settings().unwrap_or_default();
    let engine = stored
        .engine
        .as_deref()
        .and_then(Engine::parse)
        .unwrap_or(Engine::Cloud);
    let base_url = stored
        .engine_url
        .filter(|u| !u.trim().is_empty())
        .unwrap_or_else(|| engine.default_base_url().to_string());
    let credentials = match engine {
        Engine::Cloud => CredentialSource::Keychain,
        Engine::Local => CredentialSource::None,
    };
    let accept_expired_result_cert = stored.accept_expired_result_cert.unwrap_or(true);
    ParseConfig {
        engine,
        base_url,
        credentials,
        accept_expired_result_cert,
    }
}

/// The parser this install is configured for; every caller about to parse
/// comes through here. `MinerUCloud` refuses to exist without a token, so
/// `MissingCredentials` surfaces before a file is touched. A local server that
/// is not running is the first parse's `Offline`, not a construction error.
pub fn backend() -> Result<Box<dyn Parser>, ParseError> {
    match parse_config().engine {
        Engine::Cloud => Ok(Box::new(super::mineru::client::MinerUCloud::from_config()?)),
        Engine::Local => Ok(Box::new(super::mineru::local::MinerULocal::from_config())),
    }
}

/// The `parse` row, decoded — never a `block_on`: see `store::setting_blocking`.
pub(super) fn stored_settings() -> Option<StoredParseSettings> {
    serde_json::from_str(&crate::db::store::setting_blocking("parse")?).ok()
}
