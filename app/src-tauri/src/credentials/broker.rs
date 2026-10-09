//! The client side of `oculus-keyd` (docs/architecture.md): `has`, `store`,
//! `delete` and `forward` over `<data_dir>/keyd.sock`, one connection per call.
//!
//! `KeydError::Absent` (no socket, or nothing listening on it) means keyd is
//! not installed, and is the only error a caller may answer by reading the
//! keychain itself. Every other error surfaces.

use std::fmt;
use std::io::{BufRead, BufReader, Read, Write};
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::time::Duration;

use serde_json::{json, Value};

/// For `has`, `store` and `delete`: long enough to answer the keychain prompt
/// keyd raises on its first read after an update.
pub(crate) const OP_TIMEOUT: Duration = Duration::from_secs(60);

/// keyd's own limits on a header line and a body (`app/keyd/src/server.rs`).
const MAX_HEADER: u64 = 1024 * 1024;
const MAX_BODY: u64 = 256 * 1024 * 1024;

#[derive(Debug, Clone)]
pub(crate) struct Credentialed {
    socket: PathBuf,
}

/// Why keyd did not answer. The kinds mirror keyd's wire errors; `Broken` is
/// a connection that failed or a reply that made no sense.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum KeydError {
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
pub(crate) struct RawResponse {
    pub status: u16,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

impl RawResponse {
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers.iter().find(|(k, _)| k.eq_ignore_ascii_case(name)).map(|(_, v)| v.as_str())
    }
}

impl Credentialed {
    pub fn at(data_dir: &Path) -> Self {
        Self { socket: crate::paths::keyd_socket_path(data_dir) }
    }

    pub fn has(&self, secret: &str) -> Result<bool, KeydError> {
        let (reply, _) = self.exchange(&json!({"op": "has", "secret": secret}), &[], OP_TIMEOUT)?;
        reply.get("has").and_then(Value::as_bool).ok_or_else(|| KeydError::Broken("has: no answer".into()))
    }

    pub fn store(&self, secret: &str, value: &str) -> Result<(), KeydError> {
        self.exchange(&json!({"op": "store", "secret": secret, "value": value}), &[], OP_TIMEOUT).map(|_| ())
    }

    /// True when a value was there.
    pub fn delete(&self, secret: &str) -> Result<bool, KeydError> {
        let (reply, _) = self.exchange(&json!({"op": "delete", "secret": secret}), &[], OP_TIMEOUT)?;
        Ok(reply.get("existed").and_then(Value::as_bool).unwrap_or(false))
    }

    /// One request to `secret`'s origin with keyd adding the key. `timeout`
    /// bounds each socket read and write, as the caller's own HTTP timeout did.
    pub fn send(
        &self,
        secret: &str,
        method: &str,
        path: &str,
        headers: &[(&str, &str)],
        body: &[u8],
        timeout: Duration,
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
                    .filter_map(|pair| Some((pair.get(0)?.as_str()?.to_string(), pair.get(1)?.as_str()?.to_string())))
                    .collect()
            })
            .unwrap_or_default();
        Ok(RawResponse { status, headers, body })
    }

    /// Writes the header line and `body` straight from the caller's buffer,
    /// then reads the reply's header line and body.
    fn exchange(&self, header: &Value, body: &[u8], timeout: Duration) -> Result<(Value, Vec<u8>), KeydError> {
        let stream = match UnixStream::connect(&self.socket) {
            Ok(stream) => stream,
            Err(e) if matches!(e.kind(), std::io::ErrorKind::NotFound | std::io::ErrorKind::ConnectionRefused) => {
                return Err(KeydError::Absent)
            }
            Err(e) => return Err(KeydError::Broken(format!("connecting to {}: {e}", self.socket.display()))),
        };
        stream.set_read_timeout(Some(timeout)).ok();
        stream.set_write_timeout(Some(timeout)).ok();

        let mut line = header.to_string().into_bytes();
        line.push(b'\n');
        let mut writer = &stream;
        writer
            .write_all(&line)
            .and_then(|()| writer.write_all(body))
            .and_then(|()| writer.flush())
            .map_err(|e| KeydError::Broken(format!("sending: {e}")))?;

        let mut reader = BufReader::new(&stream);
        let mut line = Vec::new();
        (&mut reader)
            .take(MAX_HEADER)
            .read_until(b'\n', &mut line)
            .map_err(|e| KeydError::Broken(format!("reading the reply: {e}")))?;
        if line.pop() != Some(b'\n') {
            return Err(KeydError::Broken("the connection closed without a reply".into()));
        }
        let reply: Value =
            serde_json::from_slice(&line).map_err(|_| KeydError::Broken("the reply is not JSON".into()))?;
        if let Some(kind) = reply.get("error").and_then(Value::as_str) {
            let detail = reply.get("detail").and_then(Value::as_str).unwrap_or_default().to_string();
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
        let len = reply.get("body_len").and_then(Value::as_u64).unwrap_or(0);
        if len > MAX_BODY {
            return Err(KeydError::Broken(format!("a {len}-byte reply is over keyd's limit")));
        }
        let mut out = Vec::with_capacity(len as usize);
        reader
            .take(len)
            .read_to_end(&mut out)
            .map_err(|e| KeydError::Broken(format!("reading the reply: {e}")))?;
        if out.len() as u64 != len {
            return Err(KeydError::Broken("the connection closed inside the reply".into()));
        }
        Ok((reply, out))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::{FakeKeyd, Scratch};

    #[test]
    fn no_socket_or_no_listener_is_absent() {
        let dir = Scratch::new("keyd-absent");
        assert_eq!(Credentialed::at(&dir).has("voyage").unwrap_err(), KeydError::Absent);

        // A socket file launchd left behind with nothing loaded refuses the connect.
        let listener = std::os::unix::net::UnixListener::bind(dir.join("keyd.sock")).unwrap();
        drop(listener);
        assert_eq!(Credentialed::at(&dir).has("voyage").unwrap_err(), KeydError::Absent);
    }

    #[test]
    fn ops_and_their_errors_come_back_by_kind() {
        let dir = Scratch::new("keyd-ops");
        let keyd = FakeKeyd::start(&dir, |req, _| match req["op"].as_str().unwrap() {
            "has" => (json!({"has": true}), vec![]),
            "store" => (json!({"error": "keychain", "detail": "OSStatus -128"}), vec![]),
            "delete" => (json!({"existed": true}), vec![]),
            _ => (json!({"error": "nonsense"}), vec![]),
        });
        let broker = Credentialed::at(&dir);
        assert!(broker.has("voyage").unwrap());
        assert_eq!(broker.store("voyage", "pa-x").unwrap_err(), KeydError::Keychain("OSStatus -128".into()));
        assert!(broker.delete("voyage").unwrap());
        let sent = keyd.requests();
        assert_eq!(sent[1].0, json!({"op": "store", "secret": "voyage", "value": "pa-x"}));
    }

    #[test]
    fn a_forward_carries_its_body_both_ways() {
        let dir = Scratch::new("keyd-fwd");
        let keyd = FakeKeyd::start(&dir, |_, body| {
            let back: Vec<u8> = body.iter().rev().copied().collect();
            (json!({"status": 429, "headers": [["retry-after", "7"], ["content-type", "text/plain"]], "body_len": back.len()}), back)
        });
        let body: Vec<u8> = (0..2_000_000u32).map(|i| (i % 253) as u8).collect();
        let answer = Credentialed::at(&dir)
            .send("voyage", "POST", "/v1/x", &[("Content-Type", "application/json")], &body, Duration::from_secs(10))
            .unwrap();
        assert_eq!(answer.status, 429);
        assert_eq!(answer.header("Retry-After"), Some("7"));
        assert_eq!(answer.body, body.iter().rev().copied().collect::<Vec<_>>());

        let (header, sent) = &keyd.requests()[0];
        assert_eq!(header["op"], "forward");
        assert_eq!(header["path"], "/v1/x");
        assert_eq!(header["headers"], json!([["Content-Type", "application/json"]]));
        assert_eq!(header["body_len"], body.len());
        assert_eq!(sent, &body);
    }

    #[test]
    fn a_connection_that_drops_is_broken_not_absent() {
        let dir = Scratch::new("keyd-drop");
        let listener = std::os::unix::net::UnixListener::bind(dir.join("keyd.sock")).unwrap();
        std::thread::spawn(move || {
            let (stream, _) = listener.accept().unwrap();
            let mut line = String::new();
            BufReader::new(&stream).read_line(&mut line).unwrap();
            (&stream).write_all(b"{\"status\":200,\"body_len\":10}\nabc").unwrap();
        });
        let err = Credentialed::at(&dir).send("voyage", "POST", "/v1/x", &[], b"", Duration::from_secs(10)).unwrap_err();
        assert!(matches!(err, KeydError::Broken(_)), "{err:?}");
    }
}
