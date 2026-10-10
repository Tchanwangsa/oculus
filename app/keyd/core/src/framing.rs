//! The wire format, for both ends: one JSON object on a line, then
//! `body_len` raw bytes when the header has that field. Requests and replies
//! have the same shape, and a connection may carry several in turn. Any byte
//! stream carries it.

use std::fmt;
use std::io::{BufRead, Read, Write};

use serde_json::{json, Value};

/// Longest request header line keyd reads; a longer one is refused and the
/// connection closed.
pub const MAX_LINE: usize = 64 * 1024;
/// Longest reply header line a client reads: a `forward` reply lists the
/// origin's headers.
pub const MAX_REPLY_LINE: usize = 1024 * 1024;
/// Largest body either end buffers.
pub const MAX_BODY: u64 = 256 * 1024 * 1024;

/// Why a frame could not be read. The detail never quotes the stream.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FrameError(pub String);

impl fmt::Display for FrameError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// One header line, without its newline. `Ok(None)` is a clean end of stream
/// between frames.
pub fn read_line(reader: &mut impl BufRead, max: usize) -> Result<Option<Vec<u8>>, FrameError> {
    let mut line = Vec::new();
    loop {
        let buf = reader
            .fill_buf()
            .map_err(|e| FrameError(format!("read: {e}")))?;
        if buf.is_empty() {
            return if line.is_empty() {
                Ok(None)
            } else {
                Err(FrameError("the stream ended mid-line".into()))
            };
        }
        let (take, done) = match buf.iter().position(|&b| b == b'\n') {
            Some(i) => (i + 1, true),
            None => (buf.len(), false),
        };
        if line.len() + take > max + 1 {
            return Err(FrameError(format!(
                "the header line is longer than {max} bytes"
            )));
        }
        line.extend_from_slice(&buf[..take]);
        reader.consume(take);
        if done {
            line.pop();
            return Ok(Some(line));
        }
    }
}

/// The header as a JSON object, and the body length it announces.
pub fn parse_header(line: &[u8]) -> Result<(Value, u64), FrameError> {
    // serde's message is not echoed: it can quote the line, which may hold a value.
    let header: Value = serde_json::from_slice(line)
        .map_err(|_| FrameError("the header line is not JSON".into()))?;
    if !header.is_object() {
        return Err(FrameError("the header line is not a JSON object".into()));
    }
    let body_len = match header.get("body_len") {
        None => 0,
        Some(v) => v
            .as_u64()
            .ok_or_else(|| FrameError("body_len is not a length".into()))?,
    };
    if body_len > MAX_BODY {
        return Err(FrameError(format!("body_len is over {MAX_BODY} bytes")));
    }
    Ok((header, body_len))
}

pub fn read_body(reader: &mut impl Read, len: u64) -> Result<Vec<u8>, FrameError> {
    let mut body = Vec::new();
    reader
        .take(len)
        .read_to_end(&mut body)
        .map_err(|e| FrameError(format!("read: {e}")))?;
    if body.len() as u64 != len {
        return Err(FrameError("the stream ended inside the body".into()));
    }
    Ok(body)
}

/// The header line, then the body unbuffered: a body can be tens of MB. A
/// non-empty body sets `body_len`.
pub fn write_frame(w: &mut impl Write, header: &Value, body: &[u8]) -> std::io::Result<()> {
    let mut header = header.clone();
    if !body.is_empty() {
        header["body_len"] = json!(body.len());
    }
    let mut line = header.to_string().into_bytes();
    line.push(b'\n');
    w.write_all(&line)?;
    w.write_all(body)?;
    w.flush()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::BufReader;

    #[test]
    fn header_parsing_rejects_junk_without_quoting_it() {
        for line in [
            &b"not json"[..],
            b"[1]",
            b"{\"body_len\":-1}",
            b"{\"body_len\":\"x\"}",
        ] {
            assert!(
                parse_header(line).is_err(),
                "{}",
                String::from_utf8_lossy(line)
            );
        }
        let err = parse_header(b"{\"op\":\"store\",\"value\":\"pa-SECRET\"").unwrap_err();
        assert!(!err.0.contains("pa-SECRET"));
        let too_big = format!("{{\"op\":\"ping\",\"body_len\":{}}}", MAX_BODY + 1);
        assert!(parse_header(too_big.as_bytes()).is_err());
        assert_eq!(parse_header(b"{\"body_len\":5}").unwrap().1, 5);
    }

    #[test]
    fn frames_round_trip_and_the_next_one_is_not_lost() {
        let mut wire = Vec::new();
        write_frame(&mut wire, &json!({"op": "forward"}), b"hello").unwrap();
        write_frame(&mut wire, &json!({"op": "ping"}), b"").unwrap();
        let mut r = BufReader::new(&wire[..]);
        let (first, len) = parse_header(&read_line(&mut r, MAX_LINE).unwrap().unwrap()).unwrap();
        assert_eq!((first["op"].as_str(), len), (Some("forward"), 5));
        assert_eq!(read_body(&mut r, len).unwrap(), b"hello");
        let (second, len) = parse_header(&read_line(&mut r, MAX_LINE).unwrap().unwrap()).unwrap();
        assert_eq!((second["op"].as_str(), len), (Some("ping"), 0));
        assert!(
            second.get("body_len").is_none(),
            "an empty body adds no body_len"
        );
        assert_eq!(read_line(&mut r, MAX_LINE).unwrap(), None);
    }

    #[test]
    fn a_short_stream_or_an_overlong_line_is_an_error() {
        assert!(read_line(&mut BufReader::new(&b"{\"op\""[..]), MAX_LINE).is_err());
        assert!(read_body(&mut BufReader::new(&b"abc"[..]), 10).is_err());
        let long = vec![b'a'; 100];
        assert!(read_line(&mut BufReader::new(&long[..]), 64)
            .unwrap_err()
            .0
            .contains("longer than 64"));
    }
}
