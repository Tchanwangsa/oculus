//! Ed Discussion access: token auth, course mapping, thread fetching, and the
//! `<document>` XML → Markdown converter.
//!
//! Ed authenticates API calls with an `x-token` JWT, minted from the Canvas
//! session by walking the LTI 1.3 launch ([`Ed::connect_via_canvas`]);
//! `renew_token` extends it and a dead one is re-minted on the next sync.
//! `oculus auth ed <TOKEN>` is a manual override. See `docs/auth.md`.

mod client;
mod courses;
mod document;
mod lti;
mod render;
mod threads;

#[cfg(test)]
mod tests;

use std::path::PathBuf;
use std::sync::Mutex;

pub use document::document_md;

const ED_BASE: &str = "https://edstem.org/api";
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
    token: Mutex<String>,
    token_path: PathBuf,
    /// `/api/user` enrolments, fetched once per process.
    courses: Mutex<Option<Vec<EdCourse>>>,
}
