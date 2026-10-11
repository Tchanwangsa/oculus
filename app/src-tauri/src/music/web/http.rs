//! One request (a GET, or a PUT with a JSON body) against api.spotify.com
//! with a bearer token, its status read into a `WebError`. ureq rather than the session's own HTTP client, which folds
//! every non-2xx into a generic error and drops `Retry-After`. Blocking, so
//! it runs through `runtime::blocking::run`.

use std::sync::OnceLock;
use std::time::Duration;

use super::error::WebError;
use super::gate::retry_after;
use crate::runtime::blocking;

pub(super) const API: &str = "https://api.spotify.com/v1/";
/// Per request, so a stalled connection fails the click rather than hanging
/// the page.
const TIMEOUT: Duration = Duration::from_secs(20);

fn agent() -> &'static ureq::Agent {
    static AGENT: OnceLock<ureq::Agent> = OnceLock::new();
    AGENT.get_or_init(|| ureq::AgentBuilder::new().timeout(TIMEOUT).build())
}

/// One Web API request: a GET, or a PUT with a JSON body.
#[derive(Clone, Debug)]
pub(super) enum Request {
    Get(String),
    Put(String, String),
}

impl Request {
    fn url(&self) -> &str {
        match self {
            Self::Get(url) | Self::Put(url, _) => url,
        }
    }
}

/// The response body (empty for a 202 or 204). A URL that is not Spotify's
/// Web API is refused, so a `next` link from a response can only lead back
/// to it.
pub(super) async fn send(token: String, request: Request) -> Result<String, WebError> {
    if !request.url().starts_with(API) {
        return Err(WebError::Unreadable(format!(
            "a link off the Web API: {}",
            request.url()
        )));
    }
    blocking::run(move || Ok(send_blocking(&token, &request)))
        .await
        .map_err(WebError::Transport)?
}

fn send_blocking(token: &str, request: &Request) -> Result<String, WebError> {
    let auth = format!("Bearer {token}");
    let response = match request {
        Request::Get(url) => agent()
            .get(url)
            .set("Authorization", &auth)
            .set("Accept", "application/json")
            .call(),
        Request::Put(url, body) => agent()
            .put(url)
            .set("Authorization", &auth)
            .set("Content-Type", "application/json")
            .send_string(body),
    };
    match response {
        Ok(response) => response
            .into_string()
            .map_err(|e| WebError::Unreadable(e.to_string())),
        Err(ureq::Error::Status(code, response)) => {
            Err(status_error(code, response.header("Retry-After")))
        }
        Err(ureq::Error::Transport(t)) => Err(WebError::Transport(t.to_string())),
    }
}

pub(super) fn status_error(code: u16, retry: Option<&str>) -> WebError {
    match code {
        401 => WebError::Unauthorized,
        403 => WebError::Forbidden,
        429 => WebError::RateLimited(retry_after(retry).as_secs()),
        _ => WebError::Status(code),
    }
}
