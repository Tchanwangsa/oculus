//! Ed access: token auth, course mapping, Discussion threads, Lessons (slides,
//! quizzes, challenge workspaces), and the `<document>` XML → Markdown
//! converter both share.
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
mod lesson_md;
mod lessons;
mod live;
mod lti;
mod render;
mod threads;
mod workspace;

#[cfg(test)]
mod lesson_tests;
#[cfg(test)]
mod tests;

use std::sync::Mutex;

use crate::providers::credentials::Credentialed;

pub use document::document_md;
pub use lesson_md::{lesson_md, lesson_url, sorted_slides, LinkedFile, SlideExtras};
pub use lessons::LessonList;
pub use live::open_live_workspace;
pub use workspace::{unpack as unpack_workspace, WorkspaceFile};

/// Ed's API base for `data_dir`: what a launch landed on (`institution.json`),
/// else the default.
fn api_base(data_dir: &std::path::Path) -> String {
    crate::sources::institution::load(data_dir)
        .and_then(|i| i.ed_api)
        .unwrap_or_else(|| DEFAULT_API.to_string())
}

/// The API on the host a launch landed on, when that is not the default's.
fn api_for_landing(landed: &url::Url) -> Option<String> {
    let host = landed.host_str()?;
    (host != "edstem.org").then(|| format!("https://{host}/api"))
}

/// Ed's API, until a launch lands on another region (`api_for_landing`).
const DEFAULT_API: &str = "https://edstem.org/api";
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
}

pub struct Ed {
    keyd: Credentialed,
    data_dir: std::path::PathBuf,
    /// Ed's API base, from the institution profile, for the two requests that
    /// do not go through oculus-keyd: the one-shot `login_token` exchange (it
    /// has no session yet) and checking a token a person just pasted.
    api: Mutex<String>,
    /// `/api/user` enrolments, fetched once per process.
    courses: Mutex<Option<Vec<EdCourse>>>,
}
