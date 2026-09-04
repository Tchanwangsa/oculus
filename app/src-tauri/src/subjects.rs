use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter, Manager};

use crate::sync::{Engine, Silent};

pub struct SubjectsState(pub Arc<Mutex<Vec<serde_json::Value>>>);

#[tauri::command]
pub fn get_subjects(state: tauri::State<SubjectsState>) -> Vec<serde_json::Value> {
    state.0.lock().unwrap().clone()
}

/// Fetch the course list from Canvas and hand it to the frontend, which upserts
/// it into `subjects`. Emits `subjects-loaded` on success, `subjects-error`
/// otherwise — the same contract as before, now without a WebView in the middle.
#[tauri::command]
pub async fn sync_subjects(app: AppHandle) -> Result<(), String> {
    let data_dir = app.path().app_data_dir().map_err(|e| e.to_string())?;

    // Canvas can take a few seconds and this is a command, so do it off-thread
    // and let the frontend follow the events.
    std::thread::spawn(move || {
        let engine = Engine::new(&data_dir, Box::new(Silent));
        match engine.list_courses() {
            Ok(courses) => {
                let payload: Vec<serde_json::Value> =
                    courses.iter().map(crate::sync::Course::to_canvas_json).collect();
                let current = courses.iter().filter(|c| c.is_current).count();
                eprintln!(
                    "[oculus] subjects: {} total, {current} current",
                    courses.len()
                );
                *app.state::<SubjectsState>().0.lock().unwrap() = payload.clone();
                app.emit("subjects-loaded", payload).ok();
            }
            Err(e) => {
                eprintln!("[oculus] subjects failed: {e}");
                app.emit("subjects-error", e).ok();
            }
        }
    });

    Ok(())
}
