//! Which backend embeds, read from the `embed` settings row.

use serde::{Deserialize, Serialize};

use super::{voyage, EmbedError, Embedder};

/// Which backend is the embedder. An unknown value is treated as absent (so
/// `Cloud`), never guessed at. `Local` names a backend that does not ship yet;
/// the setting can express it so it need not be migrated when one does.
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

    /// Anything else — including the parse row's `"auto"` — is not an engine.
    pub(super) fn parse(value: &str) -> Option<Self> {
        match value.trim() {
            "cloud" => Some(Engine::Cloud),
            "local" => Some(Engine::Local),
            _ => None,
        }
    }
}

/// Where a backend's key comes from, if it needs one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CredentialSource {
    /// `oculus-keyd` when it is installed and the API root is Voyage's own,
    /// else the macOS keychain via `crate::providers::voyage`
    /// (`VoyageCloud::with_config`). The key never enters SQLite or the
    /// WebView.
    Keychain,
    /// Loopback to a process on this machine: nothing to authenticate.
    None,
}

impl CredentialSource {
    pub fn key(self) -> Result<Option<String>, EmbedError> {
        match self {
            CredentialSource::Keychain => {
                crate::providers::voyage::fetch_api_key().map_err(EmbedError::UnreadableCredentials)
            }
            CredentialSource::None => Ok(None),
        }
    }
}

#[derive(Debug, Clone)]
pub struct EmbedConfig {
    pub engine: Engine,
    /// API root for the chosen engine, overridable from the settings blob.
    pub base_url: String,
    pub credentials: CredentialSource,
}

/// Voyage's published API root.
pub const CLOUD_BASE_URL: &str = "https://api.voyageai.com/v1";

/// A local embedder's default origin — not the parse server's port; they are
/// separate programs.
pub const LOCAL_BASE_URL: &str = "http://127.0.0.1:9548";

/// The `embed` row, as far as this seam cares. Every field is optional because
/// the blob holds other settings too.
#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
struct StoredEmbedSettings {
    /// A string, not `Engine`, so a stale value costs this field, not the row.
    engine: Option<String>,
    engine_url: Option<String>,
}

/// Read the backend selection from the `embed` settings row. Anything
/// unreadable resolves to the cloud default.
pub fn embed_config() -> EmbedConfig {
    let stored = stored_settings().unwrap_or_default();
    let engine = stored
        .engine
        .as_deref()
        .and_then(Engine::parse)
        .unwrap_or(Engine::Cloud);
    let base_url = stored
        .engine_url
        .filter(|u| !u.trim().is_empty())
        .unwrap_or_else(|| {
            match engine {
                Engine::Cloud => CLOUD_BASE_URL,
                Engine::Local => LOCAL_BASE_URL,
            }
            .to_string()
        });
    let credentials = match engine {
        Engine::Cloud => CredentialSource::Keychain,
        Engine::Local => CredentialSource::None,
    };
    EmbedConfig {
        engine,
        base_url,
        credentials,
    }
}

/// The embedder this app's settings select. Every call site about to embed
/// goes through here rather than naming a client.
pub fn backend() -> Result<Box<dyn Embedder>, EmbedError> {
    let config = embed_config();
    match config.engine {
        Engine::Cloud => Ok(Box::new(voyage::client::VoyageCloud::with_config(&config)?)),
        // No local client exists in this process. `NotReady`, never a fallback
        // to the cloud: that would embed into a space the user did not choose.
        Engine::Local => Err(EmbedError::NotReady {
            backend: "local".into(),
        }),
    }
}

/// The `embed` row, decoded.
fn stored_settings() -> Option<StoredEmbedSettings> {
    // Reached from async Tauri commands, where a `block_on` would panic; see
    // `store::setting_blocking`.
    serde_json::from_str(&crate::db::store::setting_blocking("embed")?).ok()
}
