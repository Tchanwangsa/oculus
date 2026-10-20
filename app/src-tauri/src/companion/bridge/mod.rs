//! Between the transport and the canvas authority: authority events into
//! share logs and pushes (`events`), commits in (`commit`), snapshots and
//! catch-up out (`read`). Authority calls run on blocking threads, since a
//! commit writes the file.

pub mod commit;
pub mod events;
pub mod read;

use crate::library::files::canvases::authority::error::{AuthorityError, Code};
use hyper::StatusCode;

/// The HTTP status for an authority error.
pub fn status(code: Code) -> StatusCode {
    match code {
        Code::ConflictShape | Code::ConflictOrder | Code::ConflictBackground | Code::StaleOp => {
            StatusCode::CONFLICT
        }
        Code::Invalid => StatusCode::BAD_REQUEST,
        Code::SessionEnded => StatusCode::GONE,
        Code::TooLarge => StatusCode::PAYLOAD_TOO_LARGE,
        Code::ExternalEdit => StatusCode::LOCKED,
        Code::Io => StatusCode::INTERNAL_SERVER_ERROR,
    }
}

/// The authority's code as its wire name (`conflict-shape`, …).
pub fn code_name(failure: &AuthorityError) -> String {
    serde_json::to_value(failure.code)
        .ok()
        .and_then(|value| value.as_str().map(str::to_string))
        .unwrap_or_else(|| "io".into())
}
