//! Ed Discussion access: token auth, course mapping, thread fetching, and the
//! `<document>` XML → Markdown converter.
//!
//! Ed authenticates API calls with an `x-token` JWT that oculus-keyd holds and
//! attaches to its `ed` route; this module never reads it back. The token is
//! minted from the Canvas session by walking the LTI 1.3 launch
//! ([`Ed::connect_via_canvas`]); `renew_token` extends it and a dead one is
//! re-minted on the next sync. `oculus auth ed <TOKEN>` is a manual override.
//! See `docs/auth.md`.

mod client;
mod courses;
mod document;
mod lti;
mod render;
mod threads;

#[cfg(test)]
mod tests;

use std::sync::Mutex;

use crate::providers::credentials::Credentialed;

pub use document::document_md;

/// Ed's API origin, for the two requests that do not go through oculus-keyd:
/// the one-shot `login_token` exchange (it has no session yet) and checking a
/// token a person just pasted.
const ED_BASE: &str = "https://edstem.org/api";
/// The path prefix oculus-keyd's `ed` route takes.
const ED_API: &str = "/api";
const TIMEOUT: std::time::Duration = std::time::Duration::from_secs(60);
/// A hard stop, not a target.
const MAX_THREADS: usize = 1000;

#[derive(Debug, Clone)]
struct EdCourse {
    id: i64,
    code: String,
    year: String,
    session: String,
    created_at: String,
}

pub struct Ed {
    keyd: Credentialed,
    /// `/api/user` enrolments, fetched once per process.
    courses: Mutex<Option<Vec<EdCourse>>>,
}
