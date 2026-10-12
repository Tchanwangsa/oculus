//! Telling Ed the reader opened something in Oculus, the request its web app
//! sends on open: `?view=1` records a view on a thread and marks a lesson
//! slide seen. The sync never sends it — a scrape is not a read.

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
}

/// Record that the reader opened Ed thread or slide `id` (`kind` is
/// `"thread"` or `"slide"`).
#[tauri::command]
pub async fn ed_mark_viewed(kind: String, id: i64) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        let ed = Ed::open(&crate::library::paths::data_dir());
        match kind.as_str() {
            "thread" => ed.view_thread(id),
            "slide" => ed.view_slide(id),
            other => Err(format!("unknown Ed item kind {other}")),
        }
    })
    .await
    .map_err(|e| e.to_string())?
}
