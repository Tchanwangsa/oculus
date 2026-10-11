//! One request against the session's spclient host — a GET, or a POST of a
//! protobuf body — with the session's Login5 token and client token, its
//! status read into a `LibraryError`. ureq
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
/// A rootlist page, a search answer or a batch of track metadata is a few
/// hundred KB at most.
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

/// How a request is sent.
pub(super) enum Method {
    /// A GET accepting this content type.
    Get(&'static str),
    /// A POST of this protobuf body, with the content type
    /// `SpClient::request_with_protobuf` sets and no `Accept`, as it sends.
    PostProtobuf(Vec<u8>),
}

/// The body of the answer to `<spclient><endpoint>`. `endpoint` starts with
/// `/`.
pub(super) async fn send(
    session: &Session,
    endpoint: &str,
    method: Method,
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
    blocking::run(move || Ok(send_blocking(&url, &auth, client_token.as_deref(), method)))
        .await
        .map_err(LibraryError::Transport)?
}

fn send_blocking(
    url: &str,
    auth: &str,
    client_token: Option<&str>,
    method: Method,
) -> Result<Vec<u8>, LibraryError> {
    let request = match &method {
        Method::Get(accept) => agent().get(url).set("Accept", accept),
        Method::PostProtobuf(_) => agent().post(url).set("Content-Type", PROTOBUF),
    };
    let mut request = request.set("Authorization", auth);
    if let Some(token) = client_token {
        request = request.set("client-token", token);
    }
    let sent = match &method {
        Method::Get(_) => request.call(),
        Method::PostProtobuf(body) => request.send_bytes(body),
    };
    match sent {
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
