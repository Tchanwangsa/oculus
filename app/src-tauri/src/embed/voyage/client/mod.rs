//! The Voyage multimodal protocol, and the `Embedder` the app indexes through.
//!
//! One `POST`, one answer. `output_dtype` is precision and has no `float16`;
//! `output_encoding: "base64"` is transport and returns f32 little-endian. The
//! f16 narrowing and re-normalising happen in `embed::pack_vector`, never here.
//!
//! Errors carry a code, never the server's text (see `EmbedError`). A 429 is
//! not a failure: it is honoured, learned from (the only place the account's
//! real limits are stated) and retried without consuming an attempt.

mod embedder;
mod send;
#[cfg(test)]
mod tests;
mod wait;
mod wire;

use crate::embed::voyage::batch::Limits;
use crate::embed::voyage::ledger::{RateGate, UsageLedger};
use crate::embed::{embed_config, EmbedConfig, EmbedError};
use std::sync::Arc;

/// The `backend` this client reports, and what `Progress` is stamped with.
pub const BACKEND: &str = "voyage-cloud";

#[derive(Clone)]
pub struct VoyageCloud {
    pub(super) base_url: Arc<String>,
    pub(super) key: Arc<String>,
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
        let key = config.credentials.key()?.unwrap_or_default();
        Self::new(&config.base_url, &key)
    }

    /// `base_url` is passed in so tests can point the protocol at a local server.
    pub fn new(base_url: &str, key: &str) -> Result<Self, EmbedError> {
        let key = key.trim();
        if key.is_empty() {
            return Err(EmbedError::MissingCredentials);
        }
        Ok(Self {
            base_url: Arc::new(base_url.trim_end_matches('/').to_string()),
            key: Arc::new(key.to_string()),
            ledger: UsageLedger::shared(),
            gate: RateGate::shared(),
            limits: Limits::default(),
            time_scale: 1.0,
        })
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
