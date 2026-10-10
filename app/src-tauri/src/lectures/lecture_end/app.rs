//! The app side: the Tauri command and the startup reconcile.

use super::run::{claim, find, load, prepare, record};
use crate::harness::app::HarnessState;
use crate::lectures::lecture_jobs::{reconcile_status, spawn_job};
use tauri::{AppHandle, Emitter, State};

/// Emitted once when a run ends.
pub const LECTURE_END_EVENT: &str = "lecture-end";

#[derive(serde::Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct Finished {
    lecture_id: String,
    /// `ready` | `none` | `error`.
    status: &'static str,
    seconds: Option<u32>,
    quote: Option<String>,
    error: Option<String>,
}

/// Find where a lecture's content ends on the `lectureEnd` job's agent,
/// through the app's own harness (Codex and opencode reuse their running
/// server). Returns once claimed; the end arrives as [`LECTURE_END_EVENT`].
#[tauri::command]
pub async fn lecture_find_end(
    app: AppHandle,
    state: State<'_, HarnessState>,
    lecture_id: String,
    force: Option<bool>,
) -> Result<(), String> {
    let pool = crate::db::store::open_pool().await?;
    let lecture = load(&pool, &crate::library::paths::data_dir(), &lecture_id).await?;
    claim(
        &pool,
        &lecture,
        force.unwrap_or(false),
        "re-running replaces it",
    )
    .await?;

    let harness = state.harness.clone();
    spawn_job(
        "lecture end",
        crate::harness::jobs::Job::LectureEnd,
        move |rt, pool, selection| {
            let outcome = prepare(&lecture).and_then(|p| find(&harness, &selection, &p));
            let finished = match rt
                .block_on(record(pool, &lecture.id, &outcome))
                .and(outcome)
            {
                Ok(found) => Finished {
                    lecture_id: lecture.id.clone(),
                    status: if found.is_some() { "ready" } else { "none" },
                    seconds: found.as_ref().map(|f| f.end),
                    quote: found.map(|f| f.quote),
                    error: None,
                },
                Err(e) => {
                    eprintln!("[oculus] lecture end: {e}");
                    Finished {
                        lecture_id: lecture.id.clone(),
                        status: "error",
                        seconds: None,
                        quote: None,
                        error: Some(e),
                    }
                }
            };
            app.emit(LECTURE_END_EVENT, finished).ok();
        },
    );
    Ok(())
}

/// Startup: clear `running` left by a killed run.
pub fn reconcile(_app: &AppHandle) {
    reconcile_status("lecture end", |pool| async move {
        crate::db::store::reconcile_content_end_status(&pool).await
    });
}
