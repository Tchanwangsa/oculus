//! The multipart request to `POST /file_parse`, and what a failed status means.

use crate::parse::ParseError;
use crate::runtime::multipart::{self, FilePart};

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
    multipart::envelope(
        boundary,
        &FIELDS,
        &FilePart {
            field: "files",
            filename,
            content_type: "application/pdf",
        },
    )
}
