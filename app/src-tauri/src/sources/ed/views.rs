//! Telling Ed the reader opened something in Oculus, the requests its web
//! app sends on open: `?view=1` records a view on a thread and marks a lesson
//! slide seen, and `PUT …/complete` completes a reading slide, which Ed has
//! no other way to complete. The sync never sends either — a scrape is not a
//! read.

use super::Ed;

impl Ed {
    pub fn view_thread(&self, thread_id: i64) -> Result<(), String> {
        self.get(&format!("/threads/{thread_id}?view=1&no_comments=1"))
            .map(drop)
    }

    pub fn view_slide(&self, slide_id: i64) -> Result<(), String> {
        self.get(&format!("/lessons/slides/{slide_id}?view=1"))
            .map(drop)
    }

    pub fn complete_slide(&self, slide_id: i64) -> Result<(), String> {
        self.put(&format!("/lessons/slides/{slide_id}/complete"))
    }

    /// What Ed's web app sends on opening a slide of type `slide_type` whose
    /// status is `status`: the view, then the completion when the slide is
    /// not completed and its type completes on open. Ed's status after it.
    pub fn open_slide(
        &self,
        slide_id: i64,
        slide_type: Option<&str>,
        status: Option<&str>,
    ) -> Result<&'static str, String> {
        self.view_slide(slide_id)?;
        if status == Some("completed") {
            return Ok("completed");
        }
        if !slide_type.is_some_and(completes_on_open) {
            return Ok("seen");
        }
        self.complete_slide(slide_id)?;
        Ok("completed")
    }
}

/// Whether opening a slide of Ed type `slide_type` completes it. A challenge
/// (`code`, `jupyter` …) completes through its attempts and a quiz through
/// its answers; an unknown type is left alone.
pub fn completes_on_open(slide_type: &str) -> bool {
    matches!(
        slide_type,
        "document" | "pdf" | "video" | "html" | "webpage"
    )
}

/// Record that the reader opened Ed thread or slide `id` (`kind` is
/// `"thread"` or `"slide"`). A slide also takes its Ed `slide_type` and
/// current `status`, and answers with its status once Ed has it; a thread
/// answers `None`.
#[tauri::command]
pub async fn ed_mark_viewed(
    kind: String,
    id: i64,
    slide_type: Option<String>,
    status: Option<String>,
) -> Result<Option<String>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let ed = Ed::open(&crate::library::paths::data_dir());
        match kind.as_str() {
            "thread" => ed.view_thread(id).map(|()| None),
            "slide" => ed
                .open_slide(id, slide_type.as_deref(), status.as_deref())
                .map(|s| Some(s.to_string())),
            other => Err(format!("unknown Ed item kind {other}")),
        }
    })
    .await
    .map_err(|e| e.to_string())?
}

/// One slide's Ed status, as `ed_slide_statuses` answers.
#[derive(serde::Serialize)]
pub struct SlideStatus {
    pub id: i64,
    pub status: String,
}

/// What Ed holds as the student's status on every slide of `lesson_ids`.
/// The Lessons tab reads it on open so its counts need no sync.
#[tauri::command]
pub async fn ed_slide_statuses(lesson_ids: Vec<i64>) -> Result<Vec<SlideStatus>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let ed = Ed::open(&crate::library::paths::data_dir());
        ed.slide_statuses(&lesson_ids).map(|all| {
            all.into_iter()
                .map(|(id, status)| SlideStatus { id, status })
                .collect()
        })
    })
    .await
    .map_err(|e| e.to_string())?
}
