//! Tauri commands over the Echo360 core in `echo360.rs`.
//!
//! Everything that talks to Echo360 lives there so the CLI can use it too;
//! this file only adds the app's session cache, its paths and its events.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter, Manager};

pub use crate::echo360::Lecture as LectureData;
use crate::echo360::{self, Session};

// ── Session cache (in-memory, per course) ─────────────────────────────────────

pub struct Echo360Cache(pub Arc<Mutex<HashMap<i64, CachedSession>>>);

// ── Cancellable downloads ─────────────────────────────────────────────────────

/// One flag per in-flight download, keyed `"{media_id}:{source}"` — the same
/// key the frontend's progress bars use, because a lecture's two streams
/// download independently and cancelling one must not stop the other.
///
/// The download itself is a blocking read loop in `stream_to_file`, so there
/// is nothing to `abort()`: cancelling is a flag the loop checks between
/// chunks, and the ordinary error path then cleans up the partial file.
#[derive(Default)]
pub struct DownloadCancels(pub Mutex<HashMap<String, Arc<AtomicBool>>>);

fn cancel_key(media_id: &str, source: u8) -> String {
    format!("{media_id}:{source}")
}

pub struct CachedSession {
    session: Session,
    saved_unix: u64,
}

/// Echo360's JWT outlives a sync comfortably; re-launching LTI for every
/// request would be several round trips through Canvas each time.
const SESSION_TTL_SECS: u64 = 11 * 3600;

fn now_unix() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

fn get_or_auth(app: &AppHandle, cache: &Echo360Cache, course_id: i64) -> Result<Session, String> {
    {
        let g = cache.0.lock().unwrap();
        if let Some(c) = g.get(&course_id) {
            if now_unix() - c.saved_unix < SESSION_TTL_SECS {
                eprintln!("[oculus] echo360: reusing cached session for course {course_id}");
                return Ok(c.session.clone_fields());
            }
        }
    }
    let session = echo360::connect(&crate::auth::saved_cookie_header(app), course_id)?;
    cache.0.lock().unwrap().insert(
        course_id,
        CachedSession { session: session.clone_fields(), saved_unix: now_unix() },
    );
    Ok(session)
}

/// Remove the untrimmed partials left by an interrupted download.
pub fn cleanup_partial_downloads(app: &AppHandle) {
    if let Ok(dir) = app.path().app_data_dir() {
        echo360::cleanup_partial_downloads(&dir);
    }
}

// ── Commands ──────────────────────────────────────────────────────────────────

#[tauri::command]
pub async fn echo360_sync_lectures(
    app: AppHandle,
    cache: tauri::State<'_, Echo360Cache>,
    canvas_course_id: i64,
) -> Result<Vec<LectureData>, String> {
    let session = get_or_auth(&app, &cache, canvas_course_id)?;
    echo360::syllabus(&session)
}

/// Fetch one stream of a lecture. `source` is 1 for the Presenter screen and 2
/// for the room camera; both land in the same directory as `source<n>.mp4`,
/// and each carries its own untrimmed partial so the two can run at once.
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
    let source = source.unwrap_or(1);
    let session = get_or_auth(&app, &cache, canvas_course_id)?;
    let url = echo360::download_url(&session, &media_id, &lesson_id, source)?;

    // Registered before the first byte and removed in every exit path below,
    // so a cancel arriving for a download that already finished is a no-op
    // rather than a flag left set to poison the next attempt.
    let key = cancel_key(&media_id, source);
    let flag = Arc::new(AtomicBool::new(false));
    cancels.0.lock().unwrap().insert(key.clone(), Arc::clone(&flag));

    let dir = echo360::lecture_dir(
        &app.path().app_data_dir().map_err(|e| e.to_string())?,
        &media_id,
    );
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let raw = echo360::partial_path(&dir, source);
    let final_ = echo360::source_path(&dir, source);

    // The frontend keys its progress by media id *and* source — two streams of
    // one lecture download independently and would otherwise share a bar.
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
        let bytes = echo360::stream_to_file(
            &url,
            &raw,
            &|p| emit(p, "downloading"),
            &|| flag.load(Ordering::Relaxed),
        )?;
        eprintln!("[oculus] downloaded source {source}: {} MB", bytes / 1_000_000);

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
        // A cancellation is a finished intention, not a fault: the frontend
        // clears its bar on this phase without showing a failure, and the
        // partial megabytes are already gone above.
        emit(0, if e == echo360::CANCELLED { "cancelled" } else { "error" });
    }
    result
}

/// Ask an in-flight download to stop. Returns false when nothing was running
/// under that key — the download finished between the click and this call, or
/// it was never started.
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

/// Delete a lecture's downloaded video, freeing the disk it occupies.
///
/// Only the video goes. The transcript, chapters and recap notes are
/// kilobytes and are the expensive half to regenerate — an agent turn each,
/// against a video that re-downloads unattended. `source: None` removes both
/// streams; the directory itself stays because the transcript lives in it.
///
/// Returns the bytes freed, so the caller can say what it recovered.
#[tauri::command]
pub fn echo360_delete_video(
    app: AppHandle,
    cancels: tauri::State<'_, DownloadCancels>,
    media_id: String,
    source: Option<u8>,
) -> Result<u64, String> {
    let dir = echo360::lecture_dir(
        &app.path().app_data_dir().map_err(|e| e.to_string())?,
        &media_id,
    );

    let sources: Vec<u8> = match source {
        Some(s) => vec![s],
        None => vec![1, 2],
    };

    let mut freed = 0u64;
    for s in sources {
        // A download still running would write its file back moments after we
        // deleted it, so stop it first — deleting mid-download is exactly what
        // someone does when they realise they picked the wrong lecture.
        if let Some(flag) = cancels.0.lock().unwrap().get(&cancel_key(&media_id, s)) {
            flag.store(true, Ordering::Relaxed);
        }
        for path in [echo360::source_path(&dir, s), echo360::partial_path(&dir, s)] {
            if let Ok(meta) = std::fs::metadata(&path) {
                if std::fs::remove_file(&path).is_ok() {
                    freed += meta.len();
                }
            }
        }
    }

    eprintln!("[oculus] deleted lecture video {media_id}: {} MB freed", freed / 1_000_000);
    Ok(freed)
}

#[tauri::command]
pub async fn echo360_download_transcript(
    app: AppHandle,
    cache: tauri::State<'_, Echo360Cache>,
    lesson_id: String,
    media_id: String,
    canvas_course_id: i64,
) -> Result<String, String> {
    let session = get_or_auth(&app, &cache, canvas_course_id)?;
    let vtt = echo360::transcript(&session, &lesson_id, &media_id)?;

    let dir = echo360::lecture_dir(
        &app.path().app_data_dir().map_err(|e| e.to_string())?,
        &media_id,
    );
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let path = dir.join("transcript.vtt");
    std::fs::write(&path, vtt.as_bytes()).map_err(|e| e.to_string())?;
    eprintln!("[oculus] transcript saved: {}", path.display());
    Ok(path.to_string_lossy().to_string())
}

#[tauri::command]
pub fn echo360_read_transcript(path: String) -> Result<String, String> {
    std::fs::read_to_string(&path).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn echo360_clear_transcripts(app: AppHandle) -> Result<u32, String> {
    let dir = app
        .path()
        .app_data_dir()
        .map_err(|e| e.to_string())?
        .join("lectures");
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
}
