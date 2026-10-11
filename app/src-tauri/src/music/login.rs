//! The Spotify login: OAuth PKCE with librespot's own client id, in the
//! user's browser, with the callback served on `127.0.0.1:8898`, the redirect
//! that client id is registered for. It yields an access token, which the
//! session trades for reusable credentials on its first connect.
//!
//! librespot-oauth opens the browser itself and never hands the URL out, so
//! this drives the same `oauth2` flow directly: the port is bound before the
//! URL opens (a busy port fails at once), the URL goes through the opener
//! plugin, and the wait for the callback can be cancelled. Blocking, and its
//! token exchange uses `reqwest::blocking`, which panics on a thread that is
//! inside a tokio runtime: run it on a plain `std::thread`.

use std::io::{BufRead, BufReader, ErrorKind, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use oauth2::basic::BasicClient;
use oauth2::{
    AuthUrl, AuthorizationCode, ClientId, CsrfToken, PkceCodeChallenge, RedirectUrl, Scope,
    TokenResponse, TokenUrl,
};

pub(super) const PORT: u16 = 8898;
const REDIRECT_URI: &str = "http://127.0.0.1:8898/login";
const AUTH_URL: &str = "https://accounts.spotify.com/authorize";
const TOKEN_URL: &str = "https://accounts.spotify.com/api/token";
const POLL: Duration = Duration::from_millis(200);
/// How long a bind waits for a login this one just replaced to let go of the
/// port.
const BIND_TRIES: u32 = 10;

/// librespot's own scope list (its binary's `OAUTH_SCOPES`): the set its
/// client id is known to be granted.
const SCOPES: &[&str] = &[
    "app-remote-control",
    "playlist-modify",
    "playlist-modify-private",
    "playlist-modify-public",
    "playlist-read",
    "playlist-read-collaborative",
    "playlist-read-private",
    "streaming",
    "ugc-image-upload",
    "user-follow-modify",
    "user-follow-read",
    "user-library-modify",
    "user-library-read",
    "user-modify",
    "user-modify-playback-state",
    "user-modify-private",
    "user-personalized",
    "user-read-birthdate",
    "user-read-currently-playing",
    "user-read-email",
    "user-read-play-history",
    "user-read-playback-position",
    "user-read-playback-state",
    "user-read-private",
    "user-read-recently-played",
    "user-top-read",
];

const DONE_PAGE: &str = "<!doctype html><meta charset=utf-8><title>Oculus</title>\
<body style=\"font-family: -apple-system, sans-serif; padding: 3rem\">\
<p>Oculus is signed in to Spotify. You can close this tab.</p>";

#[derive(Debug, PartialEq)]
pub(super) enum LoginError {
    PortBusy,
    Cancelled,
    Failed(String),
}

impl std::fmt::Display for LoginError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LoginError::PortBusy => write!(f, "Port {PORT} is in use"),
            LoginError::Cancelled => f.write_str("The Spotify login was cancelled"),
            LoginError::Failed(why) => write!(f, "The Spotify login failed: {why}"),
        }
    }
}

fn failed(e: impl std::fmt::Display) -> LoginError {
    LoginError::Failed(e.to_string())
}

/// Opens the login in the browser through `open` and waits for Spotify's
/// redirect, until `cancel` is set. The access token on success.
pub(super) fn authorize(
    client_id: &str,
    cancel: &AtomicBool,
    open: impl FnOnce(&str),
) -> Result<String, LoginError> {
    let listener = bind(cancel)?;
    let client = BasicClient::new(ClientId::new(client_id.to_string()))
        .set_auth_uri(AuthUrl::new(AUTH_URL.into()).map_err(failed)?)
        .set_token_uri(TokenUrl::new(TOKEN_URL.into()).map_err(failed)?)
        .set_redirect_uri(RedirectUrl::new(REDIRECT_URI.into()).map_err(failed)?);
    let (challenge, verifier) = PkceCodeChallenge::new_random_sha256();
    let (url, csrf) = client
        .authorize_url(CsrfToken::new_random)
        .add_scopes(SCOPES.iter().map(|s| Scope::new(s.to_string())))
        .set_pkce_challenge(challenge)
        .url();
    open(url.as_str());

    let code = wait_for_code(&listener, csrf.secret(), cancel)?;
    drop(listener);

    let http = oauth2::reqwest::blocking::ClientBuilder::new()
        .redirect(oauth2::reqwest::redirect::Policy::none())
        .build()
        .map_err(failed)?;
    let token = client
        .exchange_code(AuthorizationCode::new(code))
        .set_pkce_verifier(verifier)
        .request(&http)
        .map_err(|e| failed(format!("Spotify did not issue a token ({e})")))?;
    Ok(token.access_token().secret().to_string())
}

fn bind(cancel: &AtomicBool) -> Result<TcpListener, LoginError> {
    for _ in 0..BIND_TRIES {
        match TcpListener::bind(("127.0.0.1", PORT)) {
            Ok(listener) => return Ok(listener),
            Err(e) if e.kind() == ErrorKind::AddrInUse => {}
            Err(e) => return Err(failed(format!("cannot listen on port {PORT} ({e})"))),
        }
        if cancel.load(Ordering::Relaxed) {
            return Err(LoginError::Cancelled);
        }
        std::thread::sleep(POLL);
    }
    Err(LoginError::PortBusy)
}

fn wait_for_code(
    listener: &TcpListener,
    state: &str,
    cancel: &AtomicBool,
) -> Result<String, LoginError> {
    listener.set_nonblocking(true).map_err(failed)?;
    loop {
        if cancel.load(Ordering::Relaxed) {
            return Err(LoginError::Cancelled);
        }
        match listener.accept() {
            Ok((stream, _)) => {
                if let Some(answer) = answer_callback(stream, state) {
                    return answer;
                }
            }
            Err(e) if e.kind() == ErrorKind::WouldBlock => std::thread::sleep(POLL),
            Err(e) => return Err(failed(e)),
        }
    }
}

/// One request to the listener: `None` when it is not the callback (a
/// favicon), so the wait goes on.
fn answer_callback(mut stream: TcpStream, state: &str) -> Option<Result<String, LoginError>> {
    stream.set_nonblocking(false).ok()?;
    stream.set_read_timeout(Some(Duration::from_secs(10))).ok();
    let mut line = String::new();
    BufReader::new(&stream).read_line(&mut line).ok()?;
    let target = line.split_whitespace().nth(1)?;
    let outcome = callback_outcome(target, state)?;
    let (status, body) = match &outcome {
        Ok(_) => ("200 OK", DONE_PAGE.to_string()),
        Err(e) => ("400 Bad Request", e.to_string()),
    };
    let reply = format!(
        "HTTP/1.1 {status}\r\ncontent-type: text/html; charset=utf-8\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
        body.len()
    );
    stream.write_all(reply.as_bytes()).ok();
    Some(outcome)
}

/// The code from Spotify's redirect to `/login`, checked against the `state`
/// the URL was opened with. `None` for any other path.
pub(super) fn callback_outcome(target: &str, state: &str) -> Option<Result<String, LoginError>> {
    let url = url::Url::parse(&format!("http://127.0.0.1{target}")).ok()?;
    if url.path() != "/login" {
        return None;
    }
    let param = |name: &str| {
        url.query_pairs()
            .find(|(k, _)| k == name)
            .map(|(_, v)| v.into_owned())
    };
    if let Some(error) = param("error") {
        return Some(Err(failed(format!("Spotify answered {error}"))));
    }
    if param("state").as_deref() != Some(state) {
        return Some(Err(failed("the callback's state did not match")));
    }
    Some(param("code").ok_or_else(|| failed("the callback carried no code")))
}
