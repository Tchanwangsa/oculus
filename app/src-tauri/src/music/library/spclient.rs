//! One request against the session's spclient host, or a full URL on another
//! Spotify host (search's pathfinder) — a GET, or a POST of a protobuf or JSON
//! body — with the session's Login5 token and client token, its status read
//! into a `LibraryError`. ureq
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
/// How much of a refused request's body is logged.
const REFUSAL_EXCERPT: u64 = 512;

pub(super) const PROTOBUF: &str = "application/x-protobuf";
const JSON: &str = "application/json";
/// What Spotify's web player sends with a pathfinder query.
const JSON_BODY: &str = "application/json;charset=UTF-8";

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
    /// A POST of this JSON body, accepting JSON.
    PostJson(Vec<u8>),
}

/// Where a request goes.
pub(super) enum Target<'a> {
    /// `<spclient><endpoint>`; `endpoint` starts with `/`.
    SpClient(&'a str),
    /// A full URL.
    Url(&'a str),
}

/// The body of the answer from `target`.
pub(super) async fn send(
    session: &Session,
    target: Target<'_>,
    method: Method,
) -> Result<Vec<u8>, LibraryError> {
    let spclient = session.spclient();
    let url = match target {
        Target::SpClient(endpoint) => {
            let base = spclient
                .base_url()
                .await
                .map_err(|e| LibraryError::Transport(e.to_string()))?;
            format!("{base}{endpoint}")
        }
        Target::Url(url) => url.to_string(),
    };
    let token = session
        .login5()
        .auth_token()
        .await
        .map_err(|e| LibraryError::Token(e.to_string()))?;
    // librespot goes on without one too; these endpoints answer without it.
    let client_token = spclient.client_token().await.ok();
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
        Method::PostJson(_) => agent()
            .post(url)
            .set("Content-Type", JSON_BODY)
            .set("Accept", JSON),
    };
    let mut request = request.set("Authorization", auth);
    if let Some(token) = client_token {
        request = request.set("client-token", token);
    }
    let sent = match &method {
        Method::Get(_) => request.call(),
        Method::PostProtobuf(body) | Method::PostJson(body) => request.send_bytes(body),
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
            let retry = response.header("Retry-After").map(str::to_owned);
            log_refusal(code, response);
            Err(status_error(code, retry.as_deref()))
        }
        Err(ureq::Error::Transport(t)) => Err(LibraryError::Transport(t.to_string())),
    }
}

/// The start of a non-2xx answer's body, on one line: Spotify says why it
/// refused there, and `LibraryError` keeps only the status. Logged without
/// the endpoint, as `call` does, though the body may echo a search's query.
fn log_refusal(code: u16, response: ureq::Response) {
    let content_type = response.content_type().to_owned();
    let mut body = Vec::new();
    let read = response
        .into_reader()
        .take(REFUSAL_EXCERPT)
        .read_to_end(&mut body);
    let excerpt = String::from_utf8_lossy(&body);
    match read {
        Ok(_) => elog!(
            "[music] refused {code} ({content_type}): {}",
            excerpt.trim().escape_debug()
        ),
        Err(e) => elog!("[music] refused {code} ({content_type}), body unread: {e}"),
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
