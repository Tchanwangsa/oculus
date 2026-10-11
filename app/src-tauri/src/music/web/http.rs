//! One GET against api.spotify.com with a bearer token, its status read into
//! a `WebError`. ureq rather than the session's own HTTP client, which folds
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

/// The response body. A URL that is not Spotify's Web API is refused, so a
/// `next` link from a response can only lead back to it.
pub(super) async fn get(token: String, url: String) -> Result<String, WebError> {
    if !url.starts_with(API) {
        return Err(WebError::Unreadable(format!(
            "a link off the Web API: {url}"
        )));
    }
    blocking::run(move || Ok(get_blocking(&token, &url)))
        .await
        .map_err(WebError::Transport)?
}

fn get_blocking(token: &str, url: &str) -> Result<String, WebError> {
    let response = agent()
        .get(url)
        .set("Authorization", &format!("Bearer {token}"))
        .set("Accept", "application/json")
        .call();
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
