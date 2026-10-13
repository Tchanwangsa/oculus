use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter, Manager};

use crate::sync::{Engine, Silent};

pub struct SubjectsState(pub Arc<Mutex<Vec<serde_json::Value>>>);

#[tauri::command]
pub fn get_subjects(state: tauri::State<SubjectsState>) -> Vec<serde_json::Value> {
    state.0.lock().unwrap().clone()
}

/// Startup: rows still without a `term_sort` get one from their term name, so
/// the current term is found without waiting for a course fetch.
pub fn backfill_term_sort_in_background() {
    let spawned = std::thread::Builder::new()
        .name("oculus-term-sort".into())
        .spawn(|| {
            let data_dir = crate::library::paths::data_dir();
            let southern = crate::sources::institution::load(&data_dir)
                .is_some_and(|i| i.southern_hemisphere());
            let outcome = tauri::async_runtime::block_on(async {
                let pool = crate::db::store::open_pool().await?;
                let result = crate::db::store::backfill_term_sort(&pool, southern).await;
                pool.close().await;
                result
            });
            match outcome {
                Ok(0) => {}
                Ok(n) => elog!("[oculus] subjects: gave {n} term(s) a sort key"),
                Err(e) => elog!("[oculus] subjects: term sort backfill: {e}"),
            }
        });
    if let Err(e) = spawned {
        elog!("[oculus] subjects: {e}");
    }
}

/// Fetch the course list from Canvas and hand it to the frontend, which upserts
/// it into `subjects`. Emits `subjects-loaded` on success, `subjects-error`
/// otherwise — the same contract as before, now without a WebView in the middle.
#[tauri::command]
pub async fn sync_subjects(app: AppHandle) -> Result<(), String> {
    let data_dir = crate::library::paths::data_dir();

    // Canvas can take a few seconds and this is a command, so do it off-thread
    // and let the frontend follow the events.
    std::thread::spawn(move || {
        let engine = Engine::new(&data_dir, Box::new(Silent));
        match engine.list_courses() {
            Ok(courses) => {
                let payload: Vec<serde_json::Value> = courses
                    .iter()
                    .map(crate::sync::Course::to_canvas_json)
                    .collect();
                let current = courses.iter().filter(|c| c.is_current).count();
                elog!(
                    "[oculus] subjects: {} total, {current} current",
                    courses.len()
                );
                *app.state::<SubjectsState>().0.lock().unwrap() = payload.clone();
                app.emit("subjects-loaded", payload).ok();
            }
            Err(e) => {
                elog!("[oculus] subjects failed: {e}");
                app.emit("subjects-error", e).ok();
            }
        }
    });

    Ok(())
}
