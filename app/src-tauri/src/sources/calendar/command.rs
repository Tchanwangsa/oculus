//! The Tauri command the frontend syncs a course's calendar through.

use super::fetch::fetch;
use super::CalendarEvent;

use crate::sources::canvas::Canvas;

/// Fetch one course's calendar; the frontend's `upsertCalendarEvents` stores it.
#[tauri::command]
pub async fn calendar_sync_events(canvas_course_id: i64) -> Result<Vec<CalendarEvent>, String> {
    crate::runtime::blocking::run(move || {
        let canvas = Canvas::open(&crate::library::paths::data_dir());
        canvas.check_keyd()?;
        fetch(&canvas, canvas_course_id)
    })
    .await
}
