//! Tauri commands over the Echo360 core: sync a course, download, read and clear transcripts.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tauri::{AppHandle, Emitter, Manager};

use super::downloads::{cancel_key, DownloadCancels};
use super::{get_or_auth, Echo360Cache, LectureData};
use crate::sources::echo360;

#[tauri::command]
pub async fn echo360_sync_lectures(
    cache: tauri::State<'_, Echo360Cache>,
    canvas_course_id: i64,
) -> Result<Vec<LectureData>, String> {
    let cache = cache.inner().clone();
    crate::runtime::blocking::run(move || {
        let session = get_or_auth(&cache, canvas_course_id)?;
        echo360::syllabus(&session)
    })
    .await
}

/// Fetch one stream of a lecture (`source` 1 = Presenter screen, 2 = room
/// camera); each has its own partial, so both can run at once.
#[tauri::command]
pub async fn echo360_download_video(
    app: AppHandle,
    cache: tauri::State<'_, Echo360Cache>,
    cancels: tauri::State<'_, DownloadCancels>,
    media_id: String,
    lesson_id: String,
    canvas_course_id: i64,
    source: Option<u8>,
) -> Result<String, String> {
    let cache = cache.inner().clone();
    let cancels = cancels.inner().clone();
    crate::runtime::blocking::run(move || {
        let source = source.unwrap_or(1);
        let session = get_or_auth(&cache, canvas_course_id)?;
        let url = echo360::download_url(&session, &media_id, &lesson_id, source)?;

        // Removed on every exit path, so a late cancel cannot poison a retry.
        let key = cancel_key(&media_id, source);
        let flag = Arc::new(AtomicBool::new(false));
        cancels
            .0
            .lock()
            .unwrap()
            .insert(key.clone(), Arc::clone(&flag));

        let dir = echo360::lecture_dir(&crate::library::paths::data_dir(), &media_id);
        let raw = echo360::partial_path(&dir, source);
        let final_ = echo360::source_path(&dir, source);

        let emit = |percent: u8, phase: &str| {
            app.emit(
                "lecture-download-progress",
                serde_json::json!({
                    "mediaId": &media_id,
                    "source": source,
                    "percent": percent,
                    "phase": phase,
                }),
            )
            .ok();
        };

        // On ANY error, clean up partial files so a retry starts fresh.
        let result = (|| -> Result<String, String> {
            std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
            let bytes = echo360::stream_to_file(&url, &raw, &|p| emit(p, "downloading"), &|| {
                flag.load(Ordering::Relaxed)
            })?;
            eprintln!(
                "[oculus] downloaded source {source}: {} MB",
                bytes / 1_000_000
            );

            emit(100, "trimming");
            let ffmpeg = echo360::find_ffmpeg(app.path().resource_dir().ok())
                .ok_or("ffmpeg not found — run `bun run ffmpeg` in app/")?;
            if !echo360::trim_video(&ffmpeg, &raw, &final_) {
                return Err("ffmpeg trim failed".to_string());
            }
            std::fs::remove_file(&raw).ok();

            emit(100, "complete");
            Ok(final_.to_string_lossy().to_string())
        })();

        cancels.0.lock().unwrap().remove(&key);

        if let Err(e) = &result {
            std::fs::remove_file(&raw).ok();
            std::fs::remove_file(&final_).ok();
            // Not a fault: the frontend clears its bar without showing a failure.
            emit(
                0,
                if e == echo360::CANCELLED {
                    "cancelled"
                } else {
                    "error"
                },
            );
        }
        result
    })
    .await
}

/// Ask an in-flight download to stop; false when nothing runs under that key.
#[tauri::command]
pub fn echo360_cancel_download(
    cancels: tauri::State<'_, DownloadCancels>,
    media_id: String,
    source: Option<u8>,
) -> bool {
    let key = cancel_key(&media_id, source.unwrap_or(1));
    match cancels.0.lock().unwrap().get(&key) {
        Some(flag) => {
            flag.store(true, Ordering::Relaxed);
            eprintln!("[oculus] cancelling download {key}");
            true
        }
        None => false,
    }
}

/// Delete a lecture's downloaded video (both streams when `source` is
/// `None`) and return the bytes freed. Transcript, chapters and notes stay —
/// they are the expensive half to regenerate.
#[tauri::command]
pub async fn echo360_delete_video(
    cancels: tauri::State<'_, DownloadCancels>,
    media_id: String,
    source: Option<u8>,
) -> Result<u64, String> {
    let cancels = cancels.inner().clone();
    crate::runtime::blocking::run(move || {
        let dir = echo360::lecture_dir(&crate::library::paths::data_dir(), &media_id);

        let sources: Vec<u8> = match source {
            Some(s) => vec![s],
            None => vec![1, 2],
        };

        let mut freed = 0u64;
        for s in sources {
            // A running download would write its file back, so stop it first.
            if let Some(flag) = cancels.0.lock().unwrap().get(&cancel_key(&media_id, s)) {
                flag.store(true, Ordering::Relaxed);
            }
            for path in [
                echo360::source_path(&dir, s),
                echo360::partial_path(&dir, s),
            ] {
                if let Ok(meta) = std::fs::metadata(&path) {
                    if std::fs::remove_file(&path).is_ok() {
                        freed += meta.len();
                    }
                }
            }
        }

        eprintln!(
            "[oculus] deleted lecture video {media_id}: {} MB freed",
            freed / 1_000_000
        );
        Ok(freed)
    })
    .await
}

#[tauri::command]
pub async fn echo360_download_transcript(
    cache: tauri::State<'_, Echo360Cache>,
    lesson_id: String,
    media_id: String,
    canvas_course_id: i64,
) -> Result<String, String> {
    let cache = cache.inner().clone();
    crate::runtime::blocking::run(move || {
        let session = get_or_auth(&cache, canvas_course_id)?;
        let vtt = echo360::transcript(&session, &lesson_id, &media_id)?;

        let dir = echo360::lecture_dir(&crate::library::paths::data_dir(), &media_id);
        std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        let path = dir.join("transcript.vtt");
        std::fs::write(&path, vtt.as_bytes()).map_err(|e| e.to_string())?;
        eprintln!("[oculus] transcript saved: {}", path.display());
        Ok(path.to_string_lossy().to_string())
    })
    .await
}

#[tauri::command]
pub async fn echo360_read_transcript(path: String) -> Result<String, String> {
    crate::runtime::blocking::run(move || std::fs::read_to_string(&path).map_err(|e| e.to_string()))
        .await
}

#[tauri::command]
pub async fn echo360_clear_transcripts() -> Result<u32, String> {
    crate::runtime::blocking::run(move || {
        let dir = crate::library::paths::data_dir().join("lectures");
        if !dir.exists() {
            return Ok(0);
        }
        let mut deleted = 0u32;
        if let Ok(entries) = std::fs::read_dir(&dir) {
            for entry in entries.flatten() {
                let transcript = entry.path().join("transcript.vtt");
                if transcript.exists() {
                    std::fs::remove_file(&transcript).ok();
                    deleted += 1;
                }
            }
        }
        eprintln!("[oculus] cleared {deleted} transcript files");
        Ok(deleted)
    })
    .await
}
