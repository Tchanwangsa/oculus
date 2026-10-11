//! Module-video downloads started from the app, and their cancel flags.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use tauri::{AppHandle, Emitter};

use super::AppReporter;
use crate::sources::canvas::Canvas;
use crate::sync::{Engine, Subject};

/// One cancel flag per in-flight module-video download, keyed by Canvas file
/// id; `Canvas::download_to` polls it between chunks.
#[derive(Clone, Default)]
pub struct VideoCancels(pub Arc<Mutex<HashMap<i64, Arc<AtomicBool>>>>);

/// Download one module video a sync only listed (`docs/sync.md`). Its
/// `files` row arrives as a `scrape-file` event, like any synced file;
/// progress as `canvas-video-progress`. Returns the library path.
#[tauri::command]
pub async fn canvas_download_video(
    app: AppHandle,
    cancels: tauri::State<'_, VideoCancels>,
    subject_id: i64,
    subject_code: String,
    canvas_file_id: i64,
) -> Result<String, String> {
    Canvas::open(&crate::library::paths::data_dir())
        .check_keyd()
        .map_err(|e| e.to_string())?;
    let flag = Arc::new(AtomicBool::new(false));
    {
        let mut running = cancels.0.lock().unwrap();
        if running.contains_key(&canvas_file_id) {
            return Err("That video is already downloading.".to_string());
        }
        running.insert(canvas_file_id, Arc::clone(&flag));
    }
    let cancels = cancels.inner().clone();
    crate::runtime::blocking::run(move || {
        let emit = |percent: u8, phase: &str| {
            app.emit(
                "canvas-video-progress",
                serde_json::json!({ "canvasFileId": canvas_file_id, "percent": percent, "phase": phase }),
            )
            .ok();
        };
        let engine = Engine::new(
            &crate::library::paths::data_dir(),
            Box::new(AppReporter { app: app.clone(), cancel: Arc::new(AtomicBool::new(false)) }),
        );
        let result = engine.download_video(
            &Subject { id: subject_id, code: subject_code },
            canvas_file_id,
            &|p| emit(p, "downloading"),
            &|| flag.load(Ordering::Relaxed),
        );
        // Removed on every exit path, so a late cancel cannot poison a retry.
        cancels.0.lock().unwrap().remove(&canvas_file_id);
        match &result {
            Ok(rel) => {
                eprintln!("[oculus] downloaded video {rel}");
                emit(100, "complete");
            }
            Err(e) if e == crate::sources::canvas::CANCELLED => emit(0, "cancelled"),
            Err(e) => {
                eprintln!("[oculus] video {canvas_file_id}: {e}");
                emit(0, "error");
            }
        }
        result
    })
    .await
}

/// Ask an in-flight video download to stop; false when none runs for that id.
#[tauri::command]
pub fn canvas_cancel_video(cancels: tauri::State<'_, VideoCancels>, canvas_file_id: i64) -> bool {
    match cancels.0.lock().unwrap().get(&canvas_file_id) {
        Some(flag) => {
            flag.store(true, Ordering::Relaxed);
            true
        }
        None => false,
    }
}
