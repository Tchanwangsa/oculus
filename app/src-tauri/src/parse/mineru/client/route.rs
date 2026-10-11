//! How an API call gets its token: through oculus-keyd, or with a token this
//! process holds.

use std::sync::Arc;

use super::{MinerUCloud, API_TIMEOUT};
use crate::parse::ParseError;
use crate::providers::credentials::{Credentialed, KeydError, RawResponse};
use crate::providers::ratelimit::transport_detail;

/// `CLOUD_BASE_URL`'s path, which keyd's `forward` is given in place of a URL.
pub(super) const CLOUD_PATH: &str = "/api/v4";

/// How an API call gets its token.
#[derive(Clone)]
pub(super) enum Auth {
    /// This process holds the token: keyd is absent, or the API root is not
    /// MinerU's.
    Direct(Arc<String>),
    /// keyd adds it; the token never enters this process.
    Keyd(Arc<Credentialed>),
}

/// An API call that got no answer. `Transport` backs off and retries like a
/// dropped connection; `Fatal` ends the call.
pub(super) enum Unanswered {
    Transport(String),
    Fatal(ParseError),
}

impl Unanswered {
    /// keyd's refusals in the seam's vocabulary. `upstream` and a broken
    /// socket are a failure to reach MinerU, so they back off and retry.
    pub(super) fn from_keyd(error: KeydError) -> Self {
        match error {
            KeydError::Missing(_) | KeydError::NoSession(..) => {
                Unanswered::Fatal(ParseError::MissingCredentials)
            }
            KeydError::Keychain(detail) => {
                Unanswered::Fatal(ParseError::UnreadableCredentials(detail))
            }
            KeydError::Upstream(detail) | KeydError::Broken(detail) => {
                Unanswered::Transport(detail)
            }
            KeydError::Absent => Unanswered::Transport("oculus-keyd stopped listening".into()),
            other @ (KeydError::Request(_) | KeydError::Caller(_) | KeydError::Vault(_)) => {
                Unanswered::Fatal(ParseError::Broker(other.to_string()))
            }
        }
    }
}

impl MinerUCloud {
    /// One request by this client's route. A status is an answer, whatever
    /// it is; `api_json` reads it.
    pub(super) fn request(
        &self,
        method: &str,
        path: &str,
        body: Option<&[u8]>,
    ) -> Result<RawResponse, Unanswered> {
        let mut headers = vec![("Accept", "application/json")];
        if body.is_some() {
            headers.push(("Content-Type", "application/json"));
        }
        match &self.auth {
            Auth::Keyd(broker) => broker
                .send(
                    crate::providers::mineru::SECRET,
                    method,
                    &format!("{CLOUD_PATH}{path}"),
                    &headers,
                    body.unwrap_or_default(),
                    Some(API_TIMEOUT),
                )
                .map_err(Unanswered::from_keyd),
            Auth::Direct(token) => {
                let mut request = ureq::request(method, &format!("{}{}", self.base_url, path))
                    .timeout(API_TIMEOUT)
                    .set("Authorization", &format!("Bearer {token}"));
                for (name, value) in &headers {
                    request = request.set(name, value);
                }
                let sent = match body {
                    Some(body) => request.send_bytes(body),
                    None => request.call(),
                };
                let response = match sent {
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
