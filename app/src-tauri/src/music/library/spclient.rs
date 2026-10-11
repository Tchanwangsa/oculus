//! One GET against the session's spclient host, with the session's Login5
//! token and client token, its status read into a `LibraryError`. ureq
//! rather than `SpClient::request`: librespot's HTTP client sleeps through a
//! 429's `Retry-After` and tries again, retries other failures up to ten
//! times, and folds every non-2xx into a generic error. Here a request is
//! sent once and a 429 reaches the gate. Blocking, so it runs through
//! `runtime::blocking::run`.

use std::io::Read;
use std::sync::OnceLock;
use std::time::Duration;

use librespot_core::version::{spotify_version, VERSION_STRING};
use librespot_core::Session;

use super::error::LibraryError;
use super::gate::retry_after;
use crate::runtime::blocking;

/// Per request, so a stalled connection fails the click rather than hanging
/// the page.
const TIMEOUT: Duration = Duration::from_secs(20);
/// A rootlist page or a search answer is a few hundred KB at most.
const MAX_BODY: u64 = 16 * 1024 * 1024;

pub(super) const PROTOBUF: &str = "application/x-protobuf";
pub(super) const JSON: &str = "application/json";

fn agent() -> &'static ureq::Agent {
    static AGENT: OnceLock<ureq::Agent> = OnceLock::new();
    AGENT.get_or_init(|| {
        ureq::AgentBuilder::new()
            .timeout(TIMEOUT)
            // librespot's own user agent on macOS (`core/src/http_client.rs`).
            .user_agent(&format!(
                "Spotify/{} OSX/0 ({VERSION_STRING})",
                spotify_version()
            ))
            .build()
    })
}

/// The body of `GET <spclient><endpoint>`. `endpoint` starts with `/`.
pub(super) async fn get(
    session: &Session,
    endpoint: &str,
    accept: &'static str,
) -> Result<Vec<u8>, LibraryError> {
    let spclient = session.spclient();
    let base = spclient
        .base_url()
        .await
        .map_err(|e| LibraryError::Transport(e.to_string()))?;
    let token = session
        .login5()
        .auth_token()
        .await
        .map_err(|e| LibraryError::Token(e.to_string()))?;
    // librespot goes on without one too; these endpoints answer without it.
    let client_token = spclient.client_token().await.ok();
    let url = format!("{base}{endpoint}");
    let auth = format!("{} {}", token.token_type, token.access_token);
    blocking::run(move || Ok(get_blocking(&url, &auth, client_token.as_deref(), accept)))
        .await
        .map_err(LibraryError::Transport)?
}

fn get_blocking(
    url: &str,
    auth: &str,
    client_token: Option<&str>,
    accept: &str,
) -> Result<Vec<u8>, LibraryError> {
    let mut request = agent()
        .get(url)
        .set("Authorization", auth)
        .set("Accept", accept);
    if let Some(token) = client_token {
        request = request.set("client-token", token);
    }
    match request.call() {
        Ok(response) => {
            let mut body = Vec::new();
            response
                .into_reader()
                .take(MAX_BODY)
                .read_to_end(&mut body)
                .map_err(|e| LibraryError::Transport(e.to_string()))?;
            Ok(body)
        }
        Err(ureq::Error::Status(code, response)) => {
            Err(status_error(code, response.header("Retry-After")))
        }
        Err(ureq::Error::Transport(t)) => Err(LibraryError::Transport(t.to_string())),
    }
}

pub(super) fn status_error(code: u16, retry: Option<&str>) -> LibraryError {
    match code {
        401 => LibraryError::Unauthorized,
        403 => LibraryError::Forbidden,
        429 => LibraryError::RateLimited(retry_after(retry).as_secs()),
        _ => LibraryError::Status(code),
    }
}
