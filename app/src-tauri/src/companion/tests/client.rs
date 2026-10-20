//! A raw HTTP/1.1 client (one request per connection) and a WebSocket client.

use serde_json::Value;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::WebSocketStream;

pub struct Reply {
    pub status: u16,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

impl Reply {
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(n, _)| n.eq_ignore_ascii_case(name))
            .map(|(_, v)| v.as_str())
    }

    pub fn json(&self) -> Value {
        serde_json::from_slice(&self.body).unwrap_or_else(|_| {
            panic!(
                "not JSON ({}): {}",
                self.status,
                String::from_utf8_lossy(&self.body)
            )
        })
    }

    pub fn text(&self) -> String {
        String::from_utf8_lossy(&self.body).into_owned()
    }

    /// The whole response as text, for "never echoed" checks.
    pub fn everything(&self) -> String {
        let headers: Vec<String> = self
            .headers
            .iter()
            .map(|(n, v)| format!("{n}: {v}"))
            .collect();
        format!("{} {}\n{}", self.status, headers.join("\n"), self.text())
    }
}

/// Build the request head; `Host` defaults to `127.0.0.1:<port>`.
fn head(
    port: u16,
    method: &str,
    path: &str,
    headers: &[(&str, &str)],
    body_len: Option<usize>,
) -> String {
    let mut text = format!("{method} {path} HTTP/1.1\r\nConnection: close\r\n");
    if !headers.iter().any(|(n, _)| n.eq_ignore_ascii_case("host")) {
        text.push_str(&format!("Host: 127.0.0.1:{port}\r\n"));
    }
    for (name, value) in headers {
        text.push_str(&format!("{name}: {value}\r\n"));
    }
    if let Some(len) = body_len {
        text.push_str(&format!("Content-Length: {len}\r\n"));
    }
    text.push_str("\r\n");
    text
}

pub async fn request(
    port: u16,
    method: &str,
    path: &str,
    headers: &[(&str, &str)],
    body: &[u8],
) -> Reply {
    let mut stream = TcpStream::connect(("127.0.0.1", port)).await.unwrap();
    let declared = !headers.iter().any(|(n, _)| {
        n.eq_ignore_ascii_case("content-length") || n.eq_ignore_ascii_case("transfer-encoding")
    });
    let len = (declared && (method == "POST" || !body.is_empty())).then_some(body.len());
    stream
        .write_all(head(port, method, path, headers, len).as_bytes())
        .await
        .unwrap();
    stream.write_all(body).await.ok();
    let mut raw = Vec::new();
    tokio::time::timeout(Duration::from_secs(20), stream.read_to_end(&mut raw))
        .await
        .expect("response in time")
        .ok();
    parse(&raw).0
}

pub async fn get(port: u16, path: &str, headers: &[(&str, &str)]) -> Reply {
    request(port, "GET", path, headers, b"").await
}

pub async fn post(port: u16, path: &str, bearer: &str, body: &Value) -> Reply {
    let body = serde_json::to_vec(body).unwrap();
    request(port, "POST", path, &[("Authorization", bearer)], &body).await
}

/// The status, headers and as much body as `raw` holds.
pub fn parse(raw: &[u8]) -> (Reply, usize) {
    let end = raw
        .windows(4)
        .position(|w| w == b"\r\n\r\n")
        .unwrap_or_else(|| panic!("no response head in {:?}", String::from_utf8_lossy(raw)));
    let head = String::from_utf8_lossy(&raw[..end]).into_owned();
    let mut lines = head.split("\r\n");
    let status = lines
        .next()
        .and_then(|line| line.split(' ').nth(1))
        .and_then(|code| code.parse().ok())
        .unwrap();
    let headers = lines
        .filter_map(|line| line.split_once(':'))
        .map(|(n, v)| (n.trim().to_string(), v.trim().to_string()))
        .collect();
    (
        Reply {
            status,
            headers,
            body: raw[end + 4..].to_vec(),
        },
        end + 4,
    )
}

/// Send a GET and read only its head, leaving the body on the socket.
pub async fn open_response(port: u16, path: &str, headers: &[(&str, &str)]) -> (TcpStream, Reply) {
    let mut stream = TcpStream::connect(("127.0.0.1", port)).await.unwrap();
    stream
        .write_all(head(port, "GET", path, headers, None).as_bytes())
        .await
        .unwrap();
    let mut raw = Vec::new();
    let mut buf = [0u8; 1024];
    while !raw.windows(4).any(|w| w == b"\r\n\r\n") {
        let n = stream.read(&mut buf).await.unwrap();
        assert!(n > 0, "closed before the head");
        raw.extend_from_slice(&buf[..n]);
    }
    (stream, parse(&raw).0)
}

pub type Socket = WebSocketStream<TcpStream>;

pub async fn socket(
    port: u16,
    share_id: &str,
) -> Result<Socket, tokio_tungstenite::tungstenite::Error> {
    let stream = TcpStream::connect(("127.0.0.1", port)).await.unwrap();
    let url = format!("ws://127.0.0.1:{port}/api/s/{share_id}/ws");
    tokio_tungstenite::client_async(url, stream)
        .await
        .map(|(socket, _)| socket)
}

/// Connect, say hello, and return the socket with its `welcome`.
pub async fn joined(port: u16, share_id: &str, secret: &str, client: &str) -> (Socket, Value) {
    use futures_util::SinkExt;
    let mut socket = socket(port, share_id).await.unwrap();
    let hello = serde_json::json!({"type": "hello", "clientId": client, "secret": secret});
    socket.send(Message::text(hello.to_string())).await.unwrap();
    let welcome = next_json(&mut socket).await.expect("welcome");
    assert_eq!(welcome["type"], "welcome", "{welcome}");
    (socket, welcome)
}

/// The next text frame as JSON; `None` once the socket closes.
pub async fn next_json(socket: &mut Socket) -> Option<Value> {
    use futures_util::StreamExt;
    loop {
        let message = tokio::time::timeout(Duration::from_secs(5), socket.next())
            .await
            .expect("a frame in time");
        match message {
            Some(Ok(Message::Text(text))) => {
                return Some(serde_json::from_str(text.as_str()).unwrap())
            }
            Some(Ok(Message::Close(_))) | Some(Err(_)) | None => return None,
            Some(Ok(_)) => continue,
        }
    }
}

/// Frames until one of `kind` arrives.
pub async fn next_of(socket: &mut Socket, kind: &str) -> Option<Value> {
    loop {
        let frame = next_json(socket).await?;
        if frame["type"] == kind {
            return Some(frame);
        }
    }
}
