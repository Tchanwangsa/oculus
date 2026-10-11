//! How a request gets its key: through oculus-keyd, or with a key this
//! process holds.

use std::sync::Arc;
use std::time::Duration;

use super::VoyageCloud;
use crate::embed::EmbedError;
use crate::providers::credentials::{Credentialed, KeydError, RawResponse};
use crate::providers::ratelimit::transport_detail;

/// `/embeddings` is the text-only model's and refuses images.
pub(super) const EMBED_PATH: &str = "/multimodalembeddings";

/// `CLOUD_BASE_URL`'s path, which keyd's `forward` is given in place of a URL.
pub(super) const CLOUD_PATH: &str = "/v1";

/// Generous: a full request is tens of MB of base64 page images.
pub(super) const API_TIMEOUT: Duration = Duration::from_secs(300);

/// How a request gets its key.
#[derive(Clone)]
pub(super) enum Auth {
    /// This process holds the key: keyd is absent, or the API root is not
    /// Voyage's.
    Direct(Arc<String>),
    /// keyd adds it; the key never enters this process.
    Keyd(Arc<Credentialed>),
}

/// A request that got no answer. `Transport` backs off and retries like a
/// dropped connection; `Fatal` ends the request.
pub(super) enum Unanswered {
    Transport(String),
    Fatal(EmbedError),
}

impl Unanswered {
    /// keyd's refusals in the seam's vocabulary. `upstream` and a broken
    /// socket are a failure to reach Voyage, so they back off and retry.
    pub(super) fn from_keyd(error: KeydError) -> Self {
        match error {
            KeydError::Missing(_) | KeydError::NoSession(..) => {
                Unanswered::Fatal(EmbedError::MissingCredentials)
            }
            KeydError::Keychain(detail) => {
                Unanswered::Fatal(EmbedError::UnreadableCredentials(detail))
            }
            KeydError::Upstream(detail) | KeydError::Broken(detail) => {
                Unanswered::Transport(detail)
            }
            KeydError::Absent => Unanswered::Transport("oculus-keyd stopped listening".into()),
            other @ (KeydError::Request(_) | KeydError::Caller(_) | KeydError::Vault(_)) => {
                Unanswered::Fatal(EmbedError::Broker(other.to_string()))
            }
        }
    }
}

impl VoyageCloud {
    /// One POST of `body` by this client's route. A status is an answer,
    /// whatever it is; `send` reads it.
    pub(super) fn post(&self, body: &[u8]) -> Result<RawResponse, Unanswered> {
        let headers = [
            ("Content-Type", "application/json"),
            ("Accept", "application/json"),
        ];
        match &self.auth {
            Auth::Keyd(broker) => broker
                .send(
                    crate::providers::voyage::SECRET,
                    "POST",
                    &format!("{CLOUD_PATH}{EMBED_PATH}"),
                    &headers,
                    body,
                    Some(API_TIMEOUT),
                )
                .map_err(Unanswered::from_keyd),
            Auth::Direct(key) => {
                let mut request = ureq::post(&format!("{}{}", self.base_url, EMBED_PATH))
                    .timeout(API_TIMEOUT)
                    .set("Authorization", &format!("Bearer {key}"));
                for (name, value) in headers {
                    request = request.set(name, value);
                }
                let response = match request.send_bytes(body) {
                    Ok(response) | Err(ureq::Error::Status(_, response)) => response,
                    Err(ureq::Error::Transport(transport)) => {
                        return Err(Unanswered::Transport(transport_detail(&transport)))
                    }
                };
                let status = response.status();
                let headers = response
                    .headers_names()
                    .into_iter()
                    .filter_map(|name| {
                        let value = response.header(&name)?.to_string();
                        Some((name, value))
                    })
                    .collect();
                let body = response.into_string().unwrap_or_default().into_bytes();
                Ok(RawResponse {
                    status,
                    headers,
                    body,
                    signin: None,
                })
            }
        }
    }
}
