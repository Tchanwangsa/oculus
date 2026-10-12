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
mod front;
mod lesson_md;
mod lessons;
mod live;
mod lti;
mod palette;
mod region;
mod render;
mod threads;
mod workspace;

#[cfg(test)]
mod lesson_tests;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod thread_tests;

use std::sync::Mutex;

use crate::providers::credentials::Credentialed;

pub use document::document_md;
pub use lesson_md::{
    lesson_index_md, lesson_url, slide_md, slide_url, sorted_slides, LessonPlace, LinkedFile,
    SlideExtras, SlideFile,
};
pub use lessons::LessonList;
pub use live::open_live_workspace;
pub use threads::{thread_url, Board};
pub use workspace::{unpack as unpack_workspace, WorkspaceFile};

/// Ed's API base for `data_dir`: what a launch landed on (`institution.json`),
/// else the default.
fn api_base(data_dir: &std::path::Path) -> String {
    crate::sources::institution::load(data_dir)
        .and_then(|i| i.ed_api)
        .unwrap_or_else(|| DEFAULT_API.to_string())
}

/// Ed's web region for `data_dir`: what a launch named, else the region of
/// the API the profile holds. `None` until either is known.
fn region(data_dir: &std::path::Path) -> Option<String> {
    let inst = crate::sources::institution::load(data_dir)?;
    inst.ed_region
        .or_else(|| inst.ed_api.as_deref().and_then(region::region_of_api))
}

/// Ed's API, until a launch names the region (`region::api_for_region`).
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
    /// Top-level Discussion category names, in Ed's order (their colours).
    categories: Vec<String>,
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
