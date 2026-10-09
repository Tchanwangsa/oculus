//! The client side of `oculus-keyd`: `has`, `store`, `delete` and `forward`,
//! one connection per call, and the `ping` that `oculus keyd status` sends.
//! The app re-exports it as `credentials::Credentialed`.
//!
//! `KeydError::Absent` (nothing at the endpoint, or nothing listening on it)
//! means keyd is not installed, and is the only error a caller may answer by
//! reading the keychain itself. Every other error surfaces.

use std::fmt;
use std::io::BufReader;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use serde_json::{json, Value};

use crate::framing::{self, MAX_REPLY_LINE};
use crate::paths;
use crate::platform::{self, Conn, ConnectError};

/// For `has`, `store` and `delete`: long enough to answer the keychain prompt
/// keyd raises on its first read after an update.
pub const OP_TIMEOUT: Duration = Duration::from_secs(60);

/// For `ping`, a probe: it does no work, so a reply this late means keyd is
/// wedged. launchd's cold start is about a second and a half.
const PING_TIMEOUT: Duration = Duration::from_secs(10);

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
            KeydError::Upstream(d) => write!(f, "oculus-keyd got no answer ({d})"),
            KeydError::Request(d) => write!(f, "oculus-keyd refused the request ({d})"),
            KeydError::Caller(d) => write!(f, "oculus-keyd refused this program ({d})"),
            KeydError::Vault(d) => write!(f, "oculus-keyd could not use its vault ({d})"),
            KeydError::Broken(d) => write!(f, "oculus-keyd: {d}"),
        }
    }
}

/// What the origin answered through `forward`, whatever the status. Header
/// names are lowercase.
#[derive(Debug, Clone)]
pub struct RawResponse {
    pub status: u16,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
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

    /// One request to `secret`'s origin with keyd adding the key. `timeout`
    /// bounds each read and write, as the caller's own HTTP timeout did;
    /// `None` waits for ever, for a caller that has no timeout of its own.
    pub fn send(
        &self,
        secret: &str,
        method: &str,
        path: &str,
        headers: &[(&str, &str)],
        body: &[u8],
        timeout: Option<Duration>,
    ) -> Result<RawResponse, KeydError> {
        let header = json!({
            "op": "forward",
            "secret": secret,
            "method": method,
            "path": path,
            "headers": headers.iter().map(|(k, v)| [*k, *v]).collect::<Vec<_>>(),
            "body_len": body.len(),
        });
        let (reply, body) = self.exchange(&header, body, timeout)?;
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
        Ok(RawResponse {
            status,
            headers,
            body,
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

    /// Writes the header line and `body` straight from the caller's buffer,
    /// then reads the reply's header line and body.
    fn exchange(
        &self,
        header: &Value,
        body: &[u8],
        timeout: Option<Duration>,
    ) -> Result<(Value, Vec<u8>), KeydError> {
        let conn = (self.connect)().map_err(|e| match e {
            ConnectError::Absent(_) => KeydError::Absent,
            ConnectError::Broken(d) => {
                KeydError::Broken(format!("connecting to {}: {d}", self.endpoint.display()))
            }
        })?;
        self.over(conn, header, body, timeout)
    }

    fn over(
        &self,
        conn: Conn,
        header: &Value,
        body: &[u8],
        timeout: Option<Duration>,
    ) -> Result<(Value, Vec<u8>), KeydError> {
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
                "missing" => KeydError::Missing(detail),
                "upstream" => KeydError::Upstream(detail),
                "request" => KeydError::Request(detail),
                "caller" => KeydError::Caller(detail),
                "vault" => KeydError::Vault(detail),
                other => KeydError::Broken(format!("{other}: {detail}")),
            });
        }
        let out = framing::read_body(&mut stream, len)
            .map_err(|e| KeydError::Broken(format!("reading the reply: {e}")))?;
        Ok((reply, out))
    }
}

impl KeydError {
    /// The wire kind this error came from.
    pub fn kind(&self) -> &'static str {
        match self {
            KeydError::Absent => "absent",
            KeydError::Keychain(_) => "keychain",
            KeydError::Missing(_) => "missing",
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
        let (listener, connector) = memory::listener();
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
        (client, seen)
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
}
