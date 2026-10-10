//! The MinerU cloud protocol, and the `Parser` the app parses through.
//!
//! A batch is submitted as a list of names, MinerU answers with one signed
//! upload URL per name, each file is `PUT` to its URL, and one endpoint is
//! polled until every task reports `done` with a result zip.
//!
//! * **Errors carry a code, never the server's text** (see `ParseError`).
//! * **Failures are scoped**: only credentials, quota and a dead poll channel
//!   condemn the batch. See `Scope`.
//! * **Progress is counted**: per-task page counts are summed, never inferred.
//!
//! The two API calls go through `oculus-keyd` when it is installed and the
//! API root is MinerU's own, so this process never holds the token; otherwise
//! straight to the API root with the keychain's token (`with_config`). Both
//! routes hand `api_json` the same `RawResponse`. The signed upload PUT and
//! the result download carry no token and always go direct.

mod archive;
mod document;
mod errors;
mod extract;
mod parser;
mod protocol;
mod route;
mod run_batch;
mod task;
#[cfg(test)]
mod tests;
mod upload;

pub(in crate::parse::mineru) use archive::{find_content_list, page_count, safe_extract};
pub use document::{CloudDocument, DocumentOutput};

use crate::parse::mineru::ledger::{poll_bucket, submit_bucket, UsageLedger, MAX_PAGES_PER_TASK};
use crate::parse::{parse_config, CredentialSource, ParseConfig, ParseError, CLOUD_BASE_URL};
use crate::providers::credentials::{Credentialed, KeydError};
use crate::providers::ratelimit::TokenBucket;
use route::{Auth, Unanswered};
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

/// The `backend` stamped into every record this client writes.
pub const BACKEND: &str = "mineru-cloud";

/// Attempts per API call. A 429 deliberately does not consume one.
const ATTEMPTS: u32 = 4;
const API_TIMEOUT: Duration = Duration::from_secs(30);
/// Transfers end on a stall, never on a deadline: a 40 MB PUT over a slow
/// link takes hours, and ureq's overall `timeout` would fail its response read
/// the moment the last byte went up. Result downloads: `result_tls::agent`.
pub(super) const TRANSFER_STALL: Duration = Duration::from_secs(120);
const UPLOAD_CHUNK: usize = 1024 * 1024;
/// How long a batch may stay unfinished. Also the only bound on the 429 loop
/// in `api_json`.
const POLL_DEADLINE: Duration = Duration::from_secs(60 * 60);
const FIRST_POLL_DELAY: Duration = Duration::from_millis(2_000);
const MAX_POLL_DELAY: Duration = Duration::from_secs(10);
/// How often a parked caller re-checks for a skip while nothing changes.
const SKIP_CHECK: Duration = Duration::from_millis(300);
/// At most two `uploading` events a second per file; the last byte always reports.
const UPLOAD_REPORT_EVERY: Duration = Duration::from_millis(500);

#[derive(Clone)]
pub struct MinerUCloud {
    base_url: Arc<String>,
    auth: Auth,
    ledger: Arc<UsageLedger>,
    submit: Arc<TokenBucket>,
    poll: Arc<TokenBucket>,
    pages_per_task: u32,
    /// Every wait is multiplied by this: 1.0 in production, tiny in tests.
    time_scale: f64,
}

impl MinerUCloud {
    /// The client the app uses: engine and API root from the settings row,
    /// token through keyd or from the keychain (`with_config_in`).
    pub fn from_config() -> Result<Self, ParseError> {
        Self::with_config(&parse_config())
    }

    pub fn with_config(config: &ParseConfig) -> Result<Self, ParseError> {
        Self::with_config_in(config, &crate::library::paths::data_dir(), || {
            config.credentials.token()
        })
    }

    /// keyd when it is installed and `config` names MinerU's own API root, so
    /// keyd's fixed origin is the one meant; otherwise the token from
    /// `direct_token`. Only an absent keyd falls back. keyd's `has` keeps the
    /// early "no token saved" check.
    fn with_config_in(
        config: &ParseConfig,
        data_dir: &Path,
        direct_token: impl FnOnce() -> Result<Option<String>, ParseError>,
    ) -> Result<Self, ParseError> {
        let cloud_root = config.base_url.trim_end_matches('/') == CLOUD_BASE_URL;
        if config.credentials == CredentialSource::Keychain && cloud_root {
            let broker = Credentialed::at(data_dir);
            match broker.has(crate::providers::mineru::SECRET) {
                Ok(true) => return Ok(Self::through_keyd(broker)),
                Ok(false) => return Err(ParseError::MissingCredentials),
                Err(KeydError::Absent) => {}
                Err(error) => {
                    return Err(match Unanswered::from_keyd(error) {
                        Unanswered::Fatal(error) => error,
                        Unanswered::Transport(detail) => ParseError::Broker(detail),
                    })
                }
            }
        }
        let token = direct_token()?.unwrap_or_default();
        Self::new(&config.base_url, &token)
    }

    /// `base_url` is passed in so tests can point the protocol at a local server.
    pub fn new(base_url: &str, token: &str) -> Result<Self, ParseError> {
        let token = token.trim();
        if token.is_empty() {
            return Err(ParseError::MissingCredentials);
        }
        Ok(Self::with_auth(
            base_url,
            Auth::Direct(Arc::new(token.to_string())),
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
            submit: submit_bucket(),
            poll: poll_bucket(),
            pages_per_task: MAX_PAGES_PER_TASK,
            time_scale: 1.0,
        }
    }

    pub fn with_ledger(mut self, ledger: Arc<UsageLedger>) -> Self {
        self.ledger = ledger;
        self
    }

    pub fn with_buckets(mut self, submit: Arc<TokenBucket>, poll: Arc<TokenBucket>) -> Self {
        self.submit = submit;
        self.poll = poll;
        self
    }

    pub fn with_pages_per_task(mut self, pages: u32) -> Self {
        self.pages_per_task = pages.max(1);
        self
    }

    pub fn with_time_scale(mut self, scale: f64) -> Self {
        self.time_scale = scale;
        self
    }

    /// Which batch this client's documents may travel in: same token, same API
    /// root. Hashed so nothing printable holds the token. Through keyd the
    /// token is keyd's, so every such client shares one route identity.
    pub fn batch_key(&self) -> u64 {
        use std::hash::{Hash, Hasher};
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        self.base_url.hash(&mut hasher);
        match &self.auth {
            Auth::Direct(token) => token.hash(&mut hasher),
            Auth::Keyd(_) => "keyd".hash(&mut hasher),
        }
        hasher.finish()
    }
}
