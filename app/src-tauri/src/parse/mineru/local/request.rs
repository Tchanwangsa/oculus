//! The multipart request to `POST /file_parse`, and what a failed status means.

use crate::parse::ParseError;
use std::sync::atomic::{AtomicU64, Ordering};

use super::{BACKEND, FIELDS};

/// Only 4xx is about this document (MinerU answers 409 when the parse task
/// failed). A 5xx or 3xx is the server or the address, so it maps to the
/// retryable `NotReady` rather than the permanent `Document`.
pub(super) fn http_failure(status: u16) -> ParseError {
    if (400..500).contains(&status) {
        ParseError::Document {
            code: format!("local-http-{status}"),
        }
    } else {
        ParseError::NotReady {
            backend: BACKEND.to_string(),
        }
    }
}

/// The multipart envelope, split so the PDF streams between the two halves.
pub(super) fn envelope(boundary: &str, filename: &str) -> (Vec<u8>, Vec<u8>) {
    let mut head = String::new();
    for (name, value) in FIELDS {
        head.push_str(&format!(
            "--{boundary}\r\nContent-Disposition: form-data; name=\"{name}\"\r\n\r\n{value}\r\n"
        ));
    }
    head.push_str(&format!(
        "--{boundary}\r\nContent-Disposition: form-data; name=\"files\"; filename=\"{}\"\r\n\
         Content-Type: application/pdf\r\n\r\n",
        header_safe(filename)
    ));
    (
        head.into_bytes(),
        format!("\r\n--{boundary}--\r\n").into_bytes(),
    )
}

/// Drop the characters that could end a quoted header value early. Dropped,
/// not escaped: RFC 2183 escaping is read inconsistently, and the name only
/// labels a directory inside the ZIP.
pub(super) fn header_safe(name: &str) -> String {
    name.chars()
        .filter(|c| !matches!(c, '"' | '\\' | '\r' | '\n'))
        .collect()
}

/// Unique among concurrent requests without a random source.
pub(super) fn boundary() -> String {
    static SEQUENCE: AtomicU64 = AtomicU64::new(0);
    let nanos = crate::runtime::clock::now_nanos() as u64;
    let sequence = SEQUENCE.fetch_add(1, Ordering::Relaxed);
    format!("oculus{nanos:016x}{:08x}{sequence:08x}", std::process::id())
}
