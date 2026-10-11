//! The Voyage multimodal protocol, and the `Embedder` the app indexes through.
//!
//! One `POST`, one answer. `output_dtype` is precision and has no `float16`;
//! `output_encoding: "base64"` is transport and returns f32 little-endian. The
//! f16 narrowing and re-normalising happen in `embed::pack_vector`, never here.
//!
//! Errors carry a code, never the server's text (see `EmbedError`). A 429 is
//! not a failure: it is honoured, learned from (the only place the account's
//! real limits are stated) and retried without consuming an attempt.
//!
//! The request goes through `oculus-keyd` when it is installed and the API
//! root is Voyage's own, so this process never holds the key; otherwise
//! straight to the API root with the keychain's key (`with_config`). Both
//! routes hand `send` the same `RawResponse`.

mod embedder;
mod route;
mod send;
#[cfg(test)]
mod tests;
mod wait;
mod wire;

use crate::embed::voyage::batch::Limits;
use crate::embed::voyage::ledger::{RateGate, UsageLedger};
use crate::embed::{embed_config, CredentialSource, EmbedConfig, EmbedError, CLOUD_BASE_URL};
use crate::providers::credentials::{Credentialed, KeydError};
use route::{Auth, Unanswered};
use std::path::Path;
use std::sync::Arc;

/// The `backend` this client reports, and what `Progress` is stamped with.
pub const BACKEND: &str = "voyage-cloud";

#[derive(Clone)]
pub struct VoyageCloud {
    pub(super) base_url: Arc<String>,
    auth: Auth,
    pub(super) ledger: Arc<UsageLedger>,
    pub(super) gate: Arc<RateGate>,
    pub(super) limits: Limits,
    /// Every wait is multiplied by this; tests shrink it.
    pub(super) time_scale: f64,
}

impl VoyageCloud {
    /// The client the app uses: engine and API root from the settings row, key
    /// from the keychain.
    pub fn from_config() -> Result<Self, EmbedError> {
        Self::with_config(&embed_config())
    }

    pub fn with_config(config: &EmbedConfig) -> Result<Self, EmbedError> {
        Self::with_config_in(config, &crate::library::paths::data_dir(), || {
            config.credentials.key()
        })
    }

    /// keyd when it is installed and `config` names Voyage's own API root, so
    /// keyd's fixed origin is the one meant; otherwise the key from
    /// `direct_key`. Only an absent keyd falls back. keyd's `has` keeps the
    /// early "no key saved" check.
    fn with_config_in(
        config: &EmbedConfig,
        data_dir: &Path,
        direct_key: impl FnOnce() -> Result<Option<String>, EmbedError>,
    ) -> Result<Self, EmbedError> {
        let cloud_root = config.base_url.trim_end_matches('/') == CLOUD_BASE_URL;
        if config.credentials == CredentialSource::Keychain && cloud_root {
            let broker = Credentialed::at(data_dir);
            match broker.has(crate::providers::voyage::SECRET) {
                Ok(true) => return Ok(Self::through_keyd(broker)),
                Ok(false) => return Err(EmbedError::MissingCredentials),
                Err(KeydError::Absent) => {}
                Err(error) => {
                    return Err(match Unanswered::from_keyd(error) {
                        Unanswered::Fatal(error) => error,
                        Unanswered::Transport(detail) => EmbedError::Broker(detail),
                    })
                }
            }
        }
        let key = direct_key()?.unwrap_or_default();
        Self::new(&config.base_url, &key)
    }

    /// `base_url` is passed in so tests can point the protocol at a local server.
    pub fn new(base_url: &str, key: &str) -> Result<Self, EmbedError> {
        let key = key.trim();
        if key.is_empty() {
            return Err(EmbedError::MissingCredentials);
        }
        Ok(Self::with_auth(
            base_url,
            Auth::Direct(Arc::new(key.to_string())),
        ))
    }

    fn through_keyd(broker: Credentialed) -> Self {
        Self::with_auth(CLOUD_BASE_URL, Auth::Keyd(Arc::new(broker)))
    }

    fn with_auth(base_url: &str, auth: Auth) -> Self {
        Self {
            base_url: Arc::new(base_url.trim_end_matches('/').to_string()),
            auth,
            ledger: UsageLedger::shared(),
            gate: RateGate::shared(),
            limits: Limits::default(),
            time_scale: 1.0,
        }
    }

    pub fn with_ledger(mut self, ledger: Arc<UsageLedger>) -> Self {
        self.ledger = ledger;
        self
    }

    pub fn with_gate(mut self, gate: Arc<RateGate>) -> Self {
        self.gate = gate;
        self
    }

    pub fn with_limits(mut self, limits: Limits) -> Self {
        self.limits = limits;
        self
    }

    pub fn with_time_scale(mut self, scale: f64) -> Self {
        self.time_scale = scale;
        self
    }
}
