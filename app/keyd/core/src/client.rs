//! The client side of `oculus-keyd`: `has`, `store`, `delete` and `forward`
//! (buffered with `send`, streamed with `send_stream`), `okta_*` and
//! `ensure_signed_in` for the Okta sign-in, `session_*` for the login
//! sessions (`session`), one connection per call, and the `ping` that
//! `oculus keyd status` sends. The app re-exports it as
//! `credentials::Credentialed`.
//!
//! `KeydError::Absent` (nothing at the endpoint, or nothing listening on it)
//! means keyd is not installed, and is the only error a caller may answer by
//! going to the keychain itself. Every other error surfaces, and a login
//! session has no such fallback: it is only ever used through keyd. A sign-in
//! that keyd ran and that failed is not an error here: it is
//! `Ok(Err(LoginError))`.

use std::fmt;
use std::io::BufReader;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use serde_json::{json, Value};

use crate::framing::{self, MAX_REPLY_LINE};
use crate::okta::{outcome_from_wire, refused_signin_from_wire, LoginError, OktaStatus, Trigger};
use crate::paths;
use crate::platform::{self, Conn, ConnectError};

/// For `has`, `store`, `delete` and the `okta_*` ops: long enough to answer
/// the keychain prompt keyd raises on its first read after an update.
pub const OP_TIMEOUT: Duration = Duration::from_secs(60);

/// For `ping`, a probe: it does no work, so a reply this late means keyd is
/// wedged. launchd's cold start is about a second and a half.
const PING_TIMEOUT: Duration = Duration::from_secs(10);

/// For a request through a session route (`canvas`, `ed`). The timeout bounds
/// each read, and a rejected Canvas request goes quiet while keyd signs in:
/// up to a dozen Okta steps of 45 s each, a wait for a fresh TOTP window, and
/// possibly another caller's attempt ahead of it. Ten minutes covers the
/// worst of that; keyd's own stall detector (180 s) bounds a download.
pub const SESSION_TIMEOUT: Option<Duration> = Some(Duration::from_secs(600));

mod session;
mod stream;

#[cfg(all(test, feature = "server"))]
mod against_session;

pub use session::{SessionCookies, SessionStatus};
pub use stream::{StreamBody, StreamedResponse};

pub use crate::session::Kind as SessionKind;

type Connect = dyn Fn() -> Result<Conn, ConnectError> + Send + Sync;

#[derive(Clone)]
pub struct Client {
    endpoint: PathBuf,
    connect: Arc<Connect>,
}

impl fmt::Debug for Client {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Client")
            .field("endpoint", &self.endpoint)
            .finish_non_exhaustive()
    }
}

/// Why keyd did not answer. The kinds mirror keyd's wire errors; `Broken` is
/// a connection that failed or a reply that made no sense.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KeydError {
    Absent,
    Keychain(String),
    Missing(String),
    /// A session route had no session, and keyd's attempt to make one was
    /// refused or failed. Carries keyd's description and the sign-in's error.
    NoSession(String, LoginError),
    Upstream(String),
    Request(String),
    Caller(String),
    Vault(String),
    Broken(String),
}

impl fmt::Display for KeydError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            KeydError::Absent => f.write_str("oculus-keyd is not installed"),
            KeydError::Keychain(d) => write!(f, "the keychain refused oculus-keyd ({d})"),
            KeydError::Missing(d) => write!(f, "oculus-keyd holds no such key ({d})"),
            KeydError::NoSession(d, why) => {
                write!(
                    f,
                    "oculus-keyd has no session to use ({d}) and could not sign in: {why}"
                )
            }
            KeydError::Upstream(d) => write!(f, "oculus-keyd got no answer ({d})"),
            KeydError::Request(d) => write!(f, "oculus-keyd refused the request ({d})"),
            KeydError::Caller(d) => write!(f, "oculus-keyd refused this program ({d})"),
            KeydError::Vault(d) => write!(f, "oculus-keyd could not use its vault ({d})"),
            KeydError::Broken(d) => write!(f, "oculus-keyd: {d}"),
        }
    }
}

/// What the origin answered through `forward`, whatever the status. Header
/// names are lowercase. `signin` is set when the session route's request was
/// rejected, keyd tried to sign in and could not: the answer is the
/// rejection, and this says why there is no new session.
#[derive(Debug, Clone)]
pub struct RawResponse {
    pub status: u16,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
    pub signin: Option<LoginError>,
}

impl RawResponse {
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(name))
            .map(|(_, v)| v.as_str())
    }
}

impl Client {
    /// keyd serving `data_dir`, at its endpoint (`paths::socket`).
    pub fn at(data_dir: &Path) -> Self {
        let endpoint = paths::socket(data_dir);
        let target = endpoint.clone();
        Client {
            endpoint,
            connect: Arc::new(move || platform::connect(&target)),
        }
    }

    pub fn has(&self, secret: &str) -> Result<bool, KeydError> {
        let (reply, _) = self.exchange(
            &json!({"op": "has", "secret": secret}),
            &[],
            Some(OP_TIMEOUT),
        )?;
        reply
            .get("has")
            .and_then(Value::as_bool)
            .ok_or_else(|| KeydError::Broken("has: no answer".into()))
    }

    pub fn store(&self, secret: &str, value: &str) -> Result<(), KeydError> {
        self.exchange(
            &json!({"op": "store", "secret": secret, "value": value}),
            &[],
            Some(OP_TIMEOUT),
        )
        .map(|_| ())
    }

    /// True when a value was there.
    pub fn delete(&self, secret: &str) -> Result<bool, KeydError> {
        let (reply, _) = self.exchange(
            &json!({"op": "delete", "secret": secret}),
            &[],
            Some(OP_TIMEOUT),
        )?;
        Ok(reply
            .get("existed")
            .and_then(Value::as_bool)
            .unwrap_or(false))
    }

    /// Which Okta credentials keyd holds. The first call after an install
    /// imports the old keychain items, one prompt each, once.
    pub fn okta_status(&self) -> Result<OktaStatus, KeydError> {
        let (reply, _) = self.exchange(&json!({"op": "okta_status"}), &[], Some(OP_TIMEOUT))?;
        OktaStatus::from_wire(&reply)
            .ok_or_else(|| KeydError::Broken("okta_status: no answer".into()))
    }

    /// Saves all three. `KeydError::Request` carries keyd's validation
    /// message ("Username is required.", …) as written.
    pub fn okta_save(
        &self,
        username: &str,
        password: &str,
        totp_secret: &str,
    ) -> Result<(), KeydError> {
        self.exchange(
            &json!({"op": "okta_save", "username": username, "password": password, "totp_secret": totp_secret}),
            &[],
            Some(OP_TIMEOUT),
        )
        .map(|_| ())
    }

    /// True when any of the three was on file.
    pub fn okta_forget(&self) -> Result<bool, KeydError> {
        let (reply, _) = self.exchange(&json!({"op": "okta_forget"}), &[], Some(OP_TIMEOUT))?;
        Ok(reply
            .get("existed")
            .and_then(Value::as_bool)
            .unwrap_or(false))
    }

    /// Clears the attempt guard's failures, pause and wait, after a person
    /// signed in themselves. App only: a CLI caller gets `KeydError::Caller`,
    /// because a lockout pause is for a person to lift. A record keyd could
    /// not replace is `KeydError::Broken`.
    pub fn okta_resume(&self) -> Result<(), KeydError> {
        self.exchange(&json!({"op": "okta_resume"}), &[], Some(OP_TIMEOUT))
            .map(|_| ())
    }

    /// Has keyd sign in to Canvas through Okta, or wait for the sign-in it is
    /// already running and share its outcome. The inner result is the
    /// sign-in's own: success (the sessions are in keyd's vault, never in the
    /// reply), or the `LoginError` it ended in. There is no socket timeout: the
    /// flow takes seconds, can wait for a fresh TOTP window, and the attempt it
    /// waits on may be another caller's.
    pub fn ensure_signed_in(&self, trigger: Trigger) -> Result<Result<(), LoginError>, KeydError> {
        let (reply, _) = self.exchange(
            &json!({"op": "ensure_signed_in", "trigger": trigger.wire_name()}),
            &[],
            None,
        )?;
        outcome_from_wire(&reply)
            .ok_or_else(|| KeydError::Broken("ensure_signed_in: no outcome".into()))
    }

    /// One request to `route`'s origin (`voyage`, `mineru`, `groq`, `canvas`
    /// or `ed`) with keyd adding the credential, and the whole answer
    /// buffered. `timeout` bounds each read and write, as the caller's own
    /// HTTP timeout did; `None` waits for ever, for a caller that has no
    /// timeout of its own. A session route takes `SESSION_TIMEOUT`, because a
    /// `canvas` request that is rejected waits while keyd signs in. An answer
    /// of any status is `Ok`; a redirect is returned, never followed.
    pub fn send(
        &self,
        route: &str,
        method: &str,
        path: &str,
        headers: &[(&str, &str)],
        body: &[u8],
        timeout: Option<Duration>,
    ) -> Result<RawResponse, KeydError> {
        let header = forward_header(route, method, path, headers, body.len(), false);
        let (reply, body) = self.exchange(&header, body, timeout)?;
        let (status, headers, signin) = forward_head(&reply)?;
        Ok(RawResponse {
            status,
            headers,
            body,
            signin,
        })
    }

    /// keyd's `ping` reply (version, source hash, pid), or why there is none,
    /// in words for `oculus keyd status`. Connecting starts keyd where the
    /// OS has it registered; keyd answers without opening the vault.
    pub fn ping(&self) -> Result<Value, String> {
        let conn = (self.connect)().map_err(|e| format!("{}: {e}", self.endpoint.display()))?;
        match self.over(conn, &json!({"op": "ping"}), &[], Some(PING_TIMEOUT)) {
            Ok((reply, _)) => Ok(reply),
            Err(KeydError::Broken(d)) => Err(d),
            Err(e) => Err(format!("keyd refused ping ({}): {}", e.kind(), e.detail())),
        }
    }

    /// Opens a connection to keyd; `Absent` when nothing is there.
    fn connect(&self) -> Result<Conn, KeydError> {
        (self.connect)().map_err(|e| match e {
            ConnectError::Absent(_) => KeydError::Absent,
            ConnectError::Broken(d) => {
                KeydError::Broken(format!("connecting to {}: {d}", self.endpoint.display()))
            }
        })
    }

    /// Writes the header line and `body` straight from the caller's buffer,
    /// then reads the reply's header line and body.
    fn exchange(
        &self,
        header: &Value,
        body: &[u8],
        timeout: Option<Duration>,
    ) -> Result<(Value, Vec<u8>), KeydError> {
        let conn = self.connect()?;
        self.over(conn, header, body, timeout)
    }

    fn over(
        &self,
        conn: Conn,
        header: &Value,
        body: &[u8],
        timeout: Option<Duration>,
    ) -> Result<(Value, Vec<u8>), KeydError> {
        let (reply, len, mut stream) = self.request(conn, header, body, timeout)?;
        let out = framing::read_body(&mut stream, len)
            .map_err(|e| KeydError::Broken(format!("reading the reply: {e}")))?;
        Ok((reply, out))
    }

    /// Sends the request and reads the reply's header line, turning keyd's
    /// error replies into `KeydError`. The reader is left at the first byte
    /// after the line, `len` being the body it announced.
    fn request(
        &self,
        conn: Conn,
        header: &Value,
        body: &[u8],
        timeout: Option<Duration>,
    ) -> Result<(Value, u64, BufReader<Conn>), KeydError> {
        conn.set_timeout(timeout).ok();
        let mut stream = BufReader::new(conn);
        framing::write_frame(stream.get_mut(), header, body)
            .map_err(|e| KeydError::Broken(format!("sending: {e}")))?;

        let line = framing::read_line(&mut stream, MAX_REPLY_LINE)
            .map_err(|e| KeydError::Broken(format!("reading the reply: {e}")))?
            .ok_or_else(|| KeydError::Broken("the connection closed without a reply".into()))?;
        let (reply, len) = framing::parse_header(&line)
            .map_err(|e| KeydError::Broken(format!("the reply: {e}")))?;
        if let Some(kind) = reply.get("error").and_then(Value::as_str) {
            let detail = reply
                .get("detail")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string();
            return Err(match kind {
                "keychain" => KeydError::Keychain(detail),
                "missing" => match refused_signin_from_wire(&reply) {
                    Some(why) => KeydError::NoSession(detail, why),
                    None => KeydError::Missing(detail),
                },
                "upstream" => KeydError::Upstream(detail),
                "request" => KeydError::Request(detail),
                "caller" => KeydError::Caller(detail),
                "vault" => KeydError::Vault(detail),
                other => KeydError::Broken(format!("{other}: {detail}")),
            });
        }
        Ok((reply, len, stream))
    }
}

/// A `forward` request's header line.
fn forward_header(
    route: &str,
    method: &str,
    path: &str,
    headers: &[(&str, &str)],
    body_len: usize,
    stream: bool,
) -> Value {
    let mut header = json!({
        "op": "forward",
        "secret": route,
        "method": method,
        "path": path,
        "headers": headers.iter().map(|(k, v)| [*k, *v]).collect::<Vec<_>>(),
        "body_len": body_len,
    });
    if stream {
        header["stream"] = json!(true);
    }
    header
}

/// A `forward` reply's status, headers and refused sign-in, if any.
fn forward_head(
    reply: &Value,
) -> Result<(u16, Vec<(String, String)>, Option<LoginError>), KeydError> {
    let status = reply
        .get("status")
        .and_then(Value::as_u64)
        .and_then(|s| u16::try_from(s).ok())
        .ok_or_else(|| KeydError::Broken("forward: no status".into()))?;
    let headers = reply
        .get("headers")
        .and_then(Value::as_array)
        .map(|list| {
            list.iter()
                .filter_map(|pair| {
                    Some((
                        pair.get(0)?.as_str()?.to_string(),
                        pair.get(1)?.as_str()?.to_string(),
                    ))
                })
                .collect()
        })
        .unwrap_or_default();
    Ok((status, headers, refused_signin_from_wire(reply)))
}

impl KeydError {
    /// The wire kind this error came from.
    pub fn kind(&self) -> &'static str {
        match self {
            KeydError::Absent => "absent",
            KeydError::Keychain(_) => "keychain",
            KeydError::Missing(_) | KeydError::NoSession(..) => "missing",
            KeydError::Upstream(_) => "upstream",
            KeydError::Request(_) => "request",
            KeydError::Caller(_) => "caller",
            KeydError::Vault(_) => "vault",
            KeydError::Broken(_) => "broken",
        }
    }

    fn detail(&self) -> &str {
        match self {
            KeydError::Absent => "",
            KeydError::NoSession(d, _) => d,
            KeydError::Keychain(d)
            | KeydError::Missing(d)
            | KeydError::Upstream(d)
            | KeydError::Request(d)
            | KeydError::Caller(d)
            | KeydError::Vault(d)
            | KeydError::Broken(d) => d,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::platform::memory;
    use std::io::Write;
    use std::sync::Mutex;

    /// A client of a stand-in keyd that answers each request with `handler`,
    /// and every request it saw: header, then body.
    fn fake_keyd<H>(handler: H) -> (Client, Arc<Mutex<Vec<(Value, Vec<u8>)>>>)
    where
        H: Fn(&Value, &[u8]) -> (Value, Vec<u8>) + Send + Sync + 'static,
    {
        let (client, seen, _) = fake_keyd_timed(handler);
        (client, seen)
    }

    /// `fake_keyd`, and every timeout the client set on its connections.
    fn fake_keyd_timed<H>(
        handler: H,
    ) -> (Client, Arc<Mutex<Vec<(Value, Vec<u8>)>>>, memory::Timeouts)
    where
        H: Fn(&Value, &[u8]) -> (Value, Vec<u8>) + Send + Sync + 'static,
    {
        let (listener, connector) = memory::listener();
        let timeouts = connector.timeouts();
        let seen = Arc::new(Mutex::new(Vec::new()));
        let log = seen.clone();
        std::thread::spawn(move || loop {
            let mut stream = BufReader::new(listener.accept().unwrap());
            while let Ok(Some(line)) = framing::read_line(&mut stream, framing::MAX_LINE) {
                let (header, len) = framing::parse_header(&line).unwrap();
                let body = framing::read_body(&mut stream, len).unwrap();
                let (reply, out) = handler(&header, &body);
                log.lock().unwrap().push((header, body));
                framing::write_frame(stream.get_mut(), &reply, &out).ok();
            }
        });
        let client = Client {
            endpoint: PathBuf::from("memory"),
            connect: Arc::new(move || Ok(connector.connect())),
        };
        (client, seen, timeouts)
    }

    /// A client whose every connect fails with `error`.
    fn unreachable(error: ConnectError) -> Client {
        Client {
            endpoint: PathBuf::from("/d/keyd.sock"),
            connect: Arc::new(move || Err(error.clone())),
        }
    }

    #[test]
    fn absent_is_absent_and_any_other_connect_failure_is_broken() {
        assert_eq!(
            unreachable(ConnectError::Absent("ENOENT".into()))
                .has("voyage")
                .unwrap_err(),
            KeydError::Absent
        );
        let err = unreachable(ConnectError::Broken("EPERM".into()))
            .has("voyage")
            .unwrap_err();
        assert_eq!(
            err,
            KeydError::Broken("connecting to /d/keyd.sock: EPERM".into())
        );
    }

    #[test]
    fn ops_and_their_errors_come_back_by_kind() {
        let (broker, seen) = fake_keyd(|req, _| match req["op"].as_str().unwrap() {
            "has" => (json!({"has": true}), vec![]),
            "store" => (
                json!({"error": "keychain", "detail": "OSStatus -128"}),
                vec![],
            ),
            "delete" => (json!({"existed": true}), vec![]),
            _ => (json!({"error": "nonsense"}), vec![]),
        });
        assert!(broker.has("voyage").unwrap());
        assert_eq!(
            broker.store("voyage", "pa-x").unwrap_err(),
            KeydError::Keychain("OSStatus -128".into())
        );
        assert!(broker.delete("voyage").unwrap());
        assert!(matches!(
            broker.send("voyage", "GET", "/v1/x", &[], b"", Some(OP_TIMEOUT)),
            Err(KeydError::Broken(_))
        ));
        assert_eq!(
            seen.lock().unwrap()[1].0,
            json!({"op": "store", "secret": "voyage", "value": "pa-x"})
        );
    }

    #[test]
    fn a_forward_carries_its_body_both_ways() {
        let (broker, seen) = fake_keyd(|_, body| {
            let back: Vec<u8> = body.iter().rev().copied().collect();
            (
                json!({"status": 429, "headers": [["retry-after", "7"], ["content-type", "text/plain"]], "body_len": back.len()}),
                back,
            )
        });
        let body: Vec<u8> = (0..2_000_000u32).map(|i| (i % 253) as u8).collect();
        let answer = broker
            .send(
                "voyage",
                "POST",
                "/v1/x",
                &[("Content-Type", "application/json")],
                &body,
                Some(Duration::from_secs(10)),
            )
            .unwrap();
        assert_eq!(answer.status, 429);
        assert_eq!(answer.header("Retry-After"), Some("7"));
        assert_eq!(answer.body, body.iter().rev().copied().collect::<Vec<_>>());

        let (header, sent) = &seen.lock().unwrap()[0];
        assert_eq!(header["op"], "forward");
        assert_eq!(header["path"], "/v1/x");
        assert_eq!(
            header["headers"],
            json!([["Content-Type", "application/json"]])
        );
        assert_eq!(header["body_len"], body.len());
        assert_eq!(sent, &body);
    }

    /// A stand-in keyd that answers its one connection with `reply` and closes.
    fn answers_once(reply: &'static [u8]) -> Client {
        let (listener, connector) = memory::listener();
        std::thread::spawn(move || {
            let mut stream = BufReader::new(listener.accept().unwrap());
            framing::read_line(&mut stream, framing::MAX_LINE).unwrap();
            stream.get_mut().write_all(reply).unwrap();
        });
        Client {
            endpoint: PathBuf::from("memory"),
            connect: Arc::new(move || Ok(connector.connect())),
        }
    }

    #[test]
    fn a_connection_that_drops_is_broken_not_absent() {
        let err = answers_once(b"{\"status\":200,\"body_len\":10}\nabc")
            .send(
                "voyage",
                "POST",
                "/v1/x",
                &[],
                b"",
                Some(Duration::from_secs(10)),
            )
            .unwrap_err();
        assert!(matches!(err, KeydError::Broken(_)), "{err:?}");
        let err = answers_once(b"").has("voyage").unwrap_err();
        assert_eq!(
            err,
            KeydError::Broken("the connection closed without a reply".into())
        );
    }

    #[test]
    fn a_reply_that_never_comes_times_out_as_broken() {
        let (listener, connector) = memory::listener();
        let held = std::thread::spawn(move || listener.accept().unwrap());
        let client = Client {
            endpoint: PathBuf::from("memory"),
            connect: Arc::new(move || Ok(connector.connect())),
        };
        let started = std::time::Instant::now();
        let err = client
            .send(
                "voyage",
                "GET",
                "/v1/x",
                &[],
                b"",
                Some(Duration::from_millis(100)),
            )
            .unwrap_err();
        assert!(matches!(err, KeydError::Broken(_)), "{err:?}");
        assert!(started.elapsed() < Duration::from_secs(5));
        drop(held);
    }

    #[test]
    fn ping_returns_keyd_reply_or_its_error() {
        let reply = answers_once(b"{\"version\":\"0.1.0\",\"source_hash\":\"ab\",\"pid\":7}\n")
            .ping()
            .unwrap();
        assert_eq!(reply["pid"], 7);

        let err = answers_once(b"{\"error\":\"caller\",\"detail\":\"outside the bundle\"}\n")
            .ping()
            .unwrap_err();
        assert!(
            err.contains("caller") && err.contains("outside the bundle"),
            "{err}"
        );

        let err = unreachable(ConnectError::Absent("No such file or directory".into()))
            .ping()
            .unwrap_err();
        assert_eq!(err, "/d/keyd.sock: No such file or directory");
    }

    #[test]
    fn the_okta_ops_send_their_requests_and_read_the_answers() {
        let (broker, seen) = fake_keyd(|req, _| match req["op"].as_str().unwrap() {
            "okta_status" => (
                json!({"username": "s1234567", "has_password": true, "has_totp": false}),
                vec![],
            ),
            "okta_save" => (json!({"saved": true}), vec![]),
            "okta_forget" => (json!({"existed": true}), vec![]),
            _ => (
                json!({"result": "error", "code": "waiting", "wait_secs": 125}),
                vec![],
            ),
        });
        assert_eq!(
            broker.okta_status().unwrap(),
            OktaStatus {
                username: Some("s1234567".into()),
                has_password: true,
                has_totp: false,
            }
        );
        broker.okta_save("s1234567", "pw", "GEZD").unwrap();
        assert!(broker.okta_forget().unwrap());
        let outcome = broker.ensure_signed_in(Trigger::Startup).unwrap();
        assert_eq!(outcome, Err(LoginError::Waiting(125)));
        assert_eq!(
            outcome.unwrap_err().to_string(),
            LoginError::Waiting(125).to_string()
        );

        let seen = seen.lock().unwrap();
        let sent: Vec<&Value> = seen.iter().map(|(h, _)| h).collect();
        assert_eq!(sent[0], &json!({"op": "okta_status"}));
        assert_eq!(
            sent[1],
            &json!({"op": "okta_save", "username": "s1234567", "password": "pw", "totp_secret": "GEZD"})
        );
        assert_eq!(sent[2], &json!({"op": "okta_forget"}));
        assert_eq!(
            sent[3],
            &json!({"op": "ensure_signed_in", "trigger": "startup"})
        );
    }

    #[test]
    fn okta_resume_sends_its_op_and_surfaces_a_refusal() {
        let (broker, seen) = fake_keyd(|_, _| (json!({"resumed": true}), vec![]));
        broker.okta_resume().unwrap();
        assert_eq!(seen.lock().unwrap()[0].0, json!({"op": "okta_resume"}));

        let (broker, _) = fake_keyd(|_, _| {
            (
                json!({"error": "caller", "detail": "this op is for the Oculus app only"}),
                vec![],
            )
        });
        assert!(matches!(broker.okta_resume(), Err(KeydError::Caller(_))));
        let (broker, _) = fake_keyd(|_, _| {
            (
                json!({"error": "record", "detail": "saving sign-in.json: disk full"}),
                vec![],
            )
        });
        assert!(matches!(broker.okta_resume(), Err(KeydError::Broken(_))));
        let absent = unreachable(ConnectError::Absent("ENOENT".into()));
        assert_eq!(absent.okta_resume().unwrap_err(), KeydError::Absent);
    }

    #[test]
    fn ensure_signed_in_sets_no_socket_timeout_and_every_other_op_sets_one() {
        let (broker, _, timeouts) = fake_keyd_timed(|req, _| match req["op"].as_str().unwrap() {
            "has" => (json!({"has": true}), vec![]),
            "okta_status" => (
                json!({"username": null, "has_password": false, "has_totp": false}),
                vec![],
            ),
            "ensure_signed_in" => (json!({"result": "signed_in"}), vec![]),
            "forward" => (json!({"status": 200, "headers": [], "body_len": 0}), vec![]),
            _ => (
                json!({"stored": true, "existed": false, "saved": true}),
                vec![],
            ),
        });
        let set = || std::mem::take(&mut *timeouts.lock().unwrap());

        // A sign-in can wait for a fresh TOTP window, or on another caller's
        // attempt: it never applies a finite timeout.
        broker.ensure_signed_in(Trigger::Manual).unwrap().unwrap();
        assert_eq!(set(), [None]);

        broker.has("voyage").unwrap();
        broker.store("voyage", "pa-x").unwrap();
        broker.delete("voyage").unwrap();
        broker.okta_status().unwrap();
        broker.okta_save("u", "p", "GEZD").unwrap();
        broker.okta_forget().unwrap();
        broker.okta_resume().unwrap();
        assert_eq!(set(), [Some(OP_TIMEOUT); 7]);

        // A forward keeps the caller's own timeout, or none.
        let own = Duration::from_secs(300);
        broker
            .send("voyage", "GET", "/v1/x", &[], b"", Some(own))
            .unwrap();
        broker
            .send("voyage", "GET", "/v1/x", &[], b"", None)
            .unwrap();
        assert_eq!(set(), [Some(own), None]);
    }

    #[test]
    fn a_signed_in_outcome_is_just_success() {
        let (broker, _) = fake_keyd(|_, _| (json!({"result": "signed_in"}), vec![]));
        assert_eq!(broker.ensure_signed_in(Trigger::Manual).unwrap(), Ok(()));
    }

    #[test]
    fn a_save_refusal_keeps_keyds_message_intact() {
        let (broker, _) = fake_keyd(|_, _| {
            (
                json!({"error": "request", "detail": "Username is required."}),
                vec![],
            )
        });
        assert_eq!(
            broker.okta_save("", "pw", "GEZD").unwrap_err(),
            KeydError::Request("Username is required.".into())
        );
    }

    #[test]
    fn transport_failures_stay_errors_and_are_not_outcomes() {
        let (broker, _) = fake_keyd(|_, _| {
            (
                json!({"error": "keychain", "detail": "OSStatus -128"}),
                vec![],
            )
        });
        assert_eq!(
            broker.okta_status().unwrap_err(),
            KeydError::Keychain("OSStatus -128".into())
        );
        assert_eq!(
            broker.ensure_signed_in(Trigger::Manual).unwrap_err(),
            KeydError::Keychain("OSStatus -128".into())
        );
        // A reply this code cannot read is a broken keyd, never a sign-in result.
        let (broker, _) = fake_keyd(|_, _| (json!({"result": "from the future"}), vec![]));
        assert!(matches!(
            broker.ensure_signed_in(Trigger::Manual),
            Err(KeydError::Broken(_))
        ));
        assert!(matches!(broker.okta_status(), Err(KeydError::Broken(_))));
        let absent = unreachable(ConnectError::Absent("ENOENT".into()));
        assert_eq!(absent.okta_status().unwrap_err(), KeydError::Absent);
        assert_eq!(absent.okta_forget().unwrap_err(), KeydError::Absent);
        assert_eq!(
            absent.ensure_signed_in(Trigger::Startup).unwrap_err(),
            KeydError::Absent
        );
        assert_eq!(
            absent.okta_save("u", "p", "GEZD").unwrap_err(),
            KeydError::Absent
        );
    }
}

/// The client against the real server loop and ops, in memory.
#[cfg(all(test, feature = "server"))]
mod against_keyd {
    use super::*;
    use crate::ops::State;
    use crate::platform::memory;
    use crate::platform::Role;
    use crate::server::Server;
    use crate::test_support::okta_fake::{
        code_from_t0, script, COOKIE, PASSWORD, SEED, T0, USERNAME,
    };
    use crate::test_support::{FakeOrigin, Peers, Scratch, TestClock, BUILD};
    use crate::vault::{MasterKey, NoLegacy, StaticKey};

    fn serve(role: Role, fake: &FakeOrigin) -> (Client, Scratch, TestClock) {
        let clock = TestClock::at(T0);
        let dir = Scratch::new("client-keyd");
        let client = serve_on(role, fake, &dir, &clock);
        (client, dir, clock)
    }

    /// A keyd for `role` over `dir`, on `clock`; two of them share the data
    /// dir, and so the attempt record, as the app's and the CLI's calls do.
    fn serve_on(role: Role, fake: &FakeOrigin, dir: &Scratch, clock: &TestClock) -> Client {
        let port = fake.origin.rsplit(':').next().unwrap();
        let state = State::new(
            BUILD,
            dir.0.clone(),
            Box::new(StaticKey(MasterKey::from_bytes([5; 32]))),
            Box::new(NoLegacy),
        )
        .with_clock(clock.clock())
        .with_origins(
            Some(&format!("http://127.0.0.1:{port}")),
            Some(&format!("http://localhost:{port}")),
        )
        .unwrap();
        let (listener, connector) = memory::listener();
        let server = Arc::new(Server::new(
            state,
            Box::new(Peers::admit_as(role)),
            Duration::from_secs(60),
        ));
        std::thread::spawn(move || server.run(&[listener]));
        Client {
            endpoint: PathBuf::from("memory"),
            connect: Arc::new(move || Ok(connector.connect())),
        }
    }

    #[test]
    fn save_status_sign_in_and_forget_over_the_wire() {
        let fake = FakeOrigin::start(script(code_from_t0));
        let (keyd, dir, clock) = serve(Role::App, &fake);

        let empty = keyd.okta_status().unwrap();
        assert_eq!(
            (empty.username, empty.has_password, empty.has_totp),
            (None, false, false)
        );
        assert_eq!(
            keyd.okta_save(" ", PASSWORD, SEED).unwrap_err(),
            KeydError::Request("Username is required.".into())
        );
        assert!(matches!(
            keyd.ensure_signed_in(Trigger::Manual).unwrap(),
            Err(LoginError::NotConfigured)
        ));

        keyd.okta_save(USERNAME, PASSWORD, SEED).unwrap();
        let status = keyd.okta_status().unwrap();
        assert_eq!(status.username.as_deref(), Some(USERNAME));
        assert!(status.has_password && status.has_totp);

        assert_eq!(keyd.ensure_signed_in(Trigger::Startup).unwrap(), Ok(()));
        let cookies = keyd.session_get().unwrap();
        assert_eq!(cookies.canvas.as_deref(), Some(COOKIE));
        assert_eq!(cookies.sso.as_deref(), Some("JSESSIONID=js1; sid=sess1"));
        assert!(keyd.session_status().unwrap().authenticated);
        assert!(!paths::cookie(&dir.0).exists(), "no cookie file");
        // The guard's wait comes back as the same variant and text an
        // in-process sign-in would give.
        let Err(waiting @ LoginError::Waiting(secs)) =
            keyd.ensure_signed_in(Trigger::Browser).unwrap()
        else {
            panic!("expected the guard's wait");
        };
        assert_eq!(waiting.to_string(), LoginError::Waiting(secs).to_string());
        // A manual attempt skips that wait, but not the minute.
        assert!(matches!(
            keyd.ensure_signed_in(Trigger::Manual).unwrap(),
            Err(LoginError::Waiting(60))
        ));
        clock.advance(60);
        assert!(keyd.ensure_signed_in(Trigger::Manual).unwrap().is_ok());

        assert!(keyd.okta_forget().unwrap());
        assert!(!keyd.okta_forget().unwrap());
        assert_eq!(keyd.okta_status().unwrap().username, None);
    }

    #[test]
    fn a_rejected_password_comes_back_as_the_same_error_and_is_forgotten() {
        let fake = FakeOrigin::start(script(code_from_t0));
        let (keyd, _dir, _clock) = serve(Role::Cli, &fake);
        keyd.okta_save(USERNAME, "wrong", SEED).unwrap();
        let outcome = keyd.ensure_signed_in(Trigger::Manual).unwrap();
        let expected = LoginError::BadPassword("Password is incorrect".into());
        assert_eq!(outcome.clone().unwrap_err(), expected);
        assert_eq!(outcome.unwrap_err().to_string(), expected.to_string());
        let status = keyd.okta_status().unwrap();
        assert_eq!((status.has_password, status.has_totp), (false, true));
    }

    #[test]
    fn a_caller_of_no_role_is_refused_every_call_but_ping() {
        let fake = FakeOrigin::start(script(code_from_t0));
        let (keyd, dir, _clock) = serve(Role::Unknown, &fake);
        assert!(matches!(keyd.okta_status(), Err(KeydError::Caller(_))));
        assert!(matches!(keyd.okta_forget(), Err(KeydError::Caller(_))));
        assert!(matches!(
            keyd.okta_save(USERNAME, PASSWORD, SEED),
            Err(KeydError::Caller(_))
        ));
        assert!(matches!(
            keyd.ensure_signed_in(Trigger::Manual),
            Err(KeydError::Caller(_))
        ));
        assert!(matches!(
            keyd.has("okta.password"),
            Err(KeydError::Caller(_))
        ));
        assert!(matches!(keyd.has("voyage"), Err(KeydError::Caller(_))));
        assert!(matches!(
            keyd.store("voyage", "pa-x"),
            Err(KeydError::Caller(_))
        ));
        assert!(matches!(keyd.delete("voyage"), Err(KeydError::Caller(_))));
        assert!(matches!(
            keyd.send("voyage", "GET", "/v1/x", &[], b"", Some(OP_TIMEOUT)),
            Err(KeydError::Caller(_))
        ));
        assert!(!paths::vault(&dir.0).exists());
        assert!(fake.hits().is_empty());
        assert_eq!(keyd.ping().unwrap()["version"], BUILD.version);
    }

    #[test]
    fn a_lockout_pause_is_lifted_by_a_manual_sign_in_from_the_app_and_not_from_the_cli() {
        let fake = FakeOrigin::start(script(code_from_t0));
        let dir = Scratch::new("client-roles");
        let clock = TestClock::at(T0);
        let app = serve_on(Role::App, &fake, &dir, &clock);
        let cli = serve_on(Role::Cli, &fake, &dir, &clock);
        app.okta_save(USERNAME, PASSWORD, SEED).unwrap();
        let record = paths::sign_in_record(&dir.0);
        std::fs::create_dir_all(record.parent().unwrap()).unwrap();
        std::fs::write(
            &record,
            format!(
                r#"{{"last":{},"failures":1,"paused":"The account is locked or blocked: x","credentials_paused":true}}"#,
                T0 - 7200
            ),
        )
        .unwrap();

        let paused = cli.ensure_signed_in(Trigger::Manual).unwrap().unwrap_err();
        assert!(matches!(paused, LoginError::Paused(_)), "{paused:?}");
        // The text tells the person where to fix it.
        let text = paused.to_string();
        assert!(
            text.contains("Settings → Canvas") && text.contains("oculus auth setup"),
            "{text}"
        );
        assert!(fake.hits().is_empty());

        assert_eq!(app.ensure_signed_in(Trigger::Manual).unwrap(), Ok(()));
        clock.advance(60);
        assert_eq!(cli.ensure_signed_in(Trigger::Manual).unwrap(), Ok(()));
    }

    #[test]
    fn a_second_manual_sign_in_within_a_minute_waits_for_either_caller() {
        let fake = FakeOrigin::start(script(code_from_t0));
        let dir = Scratch::new("client-spacing");
        let clock = TestClock::at(T0);
        let app = serve_on(Role::App, &fake, &dir, &clock);
        let cli = serve_on(Role::Cli, &fake, &dir, &clock);
        app.okta_save(USERNAME, PASSWORD, SEED).unwrap();

        assert_eq!(cli.ensure_signed_in(Trigger::Manual).unwrap(), Ok(()));
        let requests = fake.hits().len();
        for client in [&cli, &app] {
            assert_eq!(
                client.ensure_signed_in(Trigger::Manual).unwrap(),
                Err(LoginError::Waiting(60))
            );
        }
        assert_eq!(fake.hits().len(), requests);
    }

    #[test]
    fn only_the_app_may_resume_the_attempt_guard_over_the_wire() {
        let fake = FakeOrigin::start(script(code_from_t0));
        let dir = Scratch::new("client-resume");
        let clock = TestClock::at(T0);
        let app = serve_on(Role::App, &fake, &dir, &clock);
        let cli = serve_on(Role::Cli, &fake, &dir, &clock);
        app.okta_save(USERNAME, PASSWORD, SEED).unwrap();
        let record = paths::sign_in_record(&dir.0);
        std::fs::create_dir_all(record.parent().unwrap()).unwrap();
        std::fs::write(&record, r#"{"last":5,"failures":3,"paused":"locked"}"#).unwrap();

        assert!(matches!(cli.okta_resume(), Err(KeydError::Caller(_))));
        assert!(matches!(
            cli.ensure_signed_in(Trigger::Startup).unwrap(),
            Err(LoginError::Paused(_))
        ));
        app.okta_resume().unwrap();
        assert_eq!(cli.ensure_signed_in(Trigger::Startup).unwrap(), Ok(()));
    }
}
