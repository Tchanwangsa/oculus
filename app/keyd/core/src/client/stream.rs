//! `forward` with the answer's body streamed instead of buffered.

use std::io::{BufReader, Read};
use std::time::Duration;

use super::{forward_head, forward_header, Client, KeydError};
use crate::platform::Conn;

/// The answer's body, read as keyd sends it. It ends when keyd closes the
/// connection, which keyd does after the origin's last byte. An origin that
/// fails partway ends it early with no error, so a caller that needs the
/// whole body compares what it read with the `content-length` header.
pub struct StreamBody(BufReader<Conn>);

impl Read for StreamBody {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        self.0.read(buf)
    }
}

/// What the origin answered through a streamed `forward`. Header names are
/// lowercase.
pub struct StreamedResponse {
    pub status: u16,
    pub headers: Vec<(String, String)>,
    pub body: StreamBody,
}

impl StreamedResponse {
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(name))
            .map(|(_, v)| v.as_str())
    }

    /// The origin's `content-length`, when it sent one.
    pub fn content_length(&self) -> Option<u64> {
        self.header("content-length")?.trim().parse().ok()
    }
}

impl std::fmt::Debug for StreamedResponse {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("StreamedResponse")
            .field("status", &self.status)
            .field("headers", &self.headers)
            .finish_non_exhaustive()
    }
}

impl Client {
    /// `send`, but the body is not buffered or capped: this returns once the
    /// origin's status and headers are in, and the body is read from the
    /// result. `timeout` bounds each read of the head and of the body, so it
    /// is the client's own stall detector. A route's `Set-Cookie` is absorbed
    /// as for `send`, and neither it nor a credential is in `headers`.
    pub fn send_stream(
        &self,
        route: &str,
        method: &str,
        path: &str,
        headers: &[(&str, &str)],
        body: &[u8],
        timeout: Option<Duration>,
    ) -> Result<StreamedResponse, KeydError> {
        let header = forward_header(route, method, path, headers, body.len(), true);
        let conn = self.connect()?;
        let (reply, _, reader) = self.request(conn, &header, body, timeout)?;
        let (status, headers) = forward_head(&reply)?;
        Ok(StreamedResponse {
            status,
            headers,
            body: StreamBody(reader),
        })
    }
}
