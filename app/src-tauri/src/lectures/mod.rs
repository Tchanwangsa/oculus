//! Tauri commands over the Echo360 core in `sources/echo360/` (shared with the CLI):
//! the app's session cache, paths and events.

pub mod chapters;
pub(crate) mod commands;
mod downloads;
pub mod lecture_end;
pub(crate) mod lecture_jobs;
pub(crate) mod media;

pub use downloads::DownloadCancels;

use crate::runtime::clock::now_secs;
pub use crate::sources::echo360::Lecture as LectureData;
use crate::sources::echo360::{self, Session};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

#[derive(Clone)]
pub struct Echo360Cache(pub Arc<Mutex<HashMap<i64, CachedSession>>>);

pub struct CachedSession {
    session: Session,
    saved_unix: u64,
}

/// Cached so each request does not re-run the LTI launch through Canvas.
const SESSION_TTL_SECS: u64 = 11 * 3600;

fn get_or_auth(cache: &Echo360Cache, course_id: i64) -> Result<Session, String> {
    {
        let g = cache.0.lock().unwrap();
        if let Some(c) = g.get(&course_id) {
            if now_secs() - c.saved_unix < SESSION_TTL_SECS {
                eprintln!("[oculus] echo360: reusing cached session for course {course_id}");
                return Ok(c.session.clone_fields());
            }
        }
    }
    let session = echo360::connect(
        &crate::sources::canvas::Canvas::open(&crate::library::paths::data_dir()),
        course_id,
    )?;
    cache.0.lock().unwrap().insert(
        course_id,
        CachedSession {
            session: session.clone_fields(),
            saved_unix: now_secs(),
        },
    );
    Ok(session)
}
