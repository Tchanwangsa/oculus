//! The app side: the three Tauri commands and the startup reconcile.

use super::frames::{grab_frame, thumbnail_pick};
use super::run::{run, Step};
use super::{Run, LIVE_GRAB_WIDTH, THUMB_WIDTH};
use crate::lectures::lecture_jobs::{check_start, reconcile_status, spawn_job, Progress};
use std::path::PathBuf;
use tauri::{AppHandle, Emitter};

/// Emitted once when a run ends. Not `lectures-changed`, which fires on
/// every playback-progress save.
pub const LECTURE_CHAPTERS_EVENT: &str = "lecture-chapters";

/// Emitted throughout a run; display only, never persisted.
pub const LECTURE_CHAPTER_PROGRESS_EVENT: &str = "lecture-chapter-progress";

#[derive(serde::Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct Finished {
    lecture_id: String,
    /// How this request ended. A refusal reports `error` while the row
    /// keeps its `ready`.
    status: &'static str,
    chapters: usize,
    error: Option<String>,
}

/// Chapter a lecture with the agent the `lectureChapters` job is
/// configured with. Returns once the run is claimed; the end arrives as
/// [`LECTURE_CHAPTERS_EVENT`].
#[tauri::command]
pub async fn lecture_find_chapters(
    app: AppHandle,
    lecture_id: String,
    force: Option<bool>,
    source: Option<u8>,
) -> Result<(), String> {
    check_start(
        &lecture_id,
        source,
        "chapter_status",
        "that lecture is already being chaptered",
    )
    .await?;

    let data_dir = crate::library::paths::data_dir();
    let force = force.unwrap_or(false);
    spawn_job(
        "chapters",
        crate::harness::jobs::Job::LectureChapters,
        move |rt, pool, selection| {
            let emit = {
                let app = app.clone();
                move |p: Progress| {
                    app.emit(LECTURE_CHAPTER_PROGRESS_EVENT, p).ok();
                }
            };

            let step = {
                let id = lecture_id.clone();
                let emit = emit.clone();
                move |s: Step| {
                    let p = match s {
                        Step::Decoding { second, duration } => Progress {
                            done: Some(second),
                            total: (duration > 0).then_some(duration),
                            ..Progress::at(&id, "decoding")
                        },
                        Step::Detected {
                            title, candidates, ..
                        } => {
                            eprintln!("[oculus] chapters: {title} — {candidates} candidate(s)");
                            Progress {
                                done: Some(0),
                                total: Some(candidates as u32 + 1),
                                ..Progress::at(&id, "frames")
                            }
                        }
                        Step::Grabbing { done, total } => Progress {
                            done: Some(done as u32),
                            total: Some(total as u32),
                            ..Progress::at(&id, "frames")
                        },
                        Step::Asking => Progress::at(&id, "agent"),
                        Step::Writing => Progress::at(&id, "writing"),
                    };
                    emit(p);
                }
            };

            // The reply is the chapter JSON, so its first delta is the agent
            // having decided: report that once as `naming`.
            let naming = std::sync::atomic::AtomicBool::new(false);
            let event = {
                let id = lecture_id.clone();
                move |ev: &crate::harness::HarnessEvent| {
                    use crate::harness::HarnessEvent as E;
                    let p = match ev {
                        E::ToolStarted { kind, title, .. } => Progress {
                            detail: Some(title.clone()),
                            kind: Some(*kind),
                            ..Progress::at(&id, "agent")
                        },
                        E::AssistantDelta { .. } | E::AssistantMessage { .. } => {
                            if naming.swap(true, std::sync::atomic::Ordering::Relaxed) {
                                return;
                            }
                            Progress::at(&id, "naming")
                        }
                        _ => return,
                    };
                    emit(p);
                }
            };

            let outcome = run(
                rt.handle(),
                pool,
                &Run {
                    data_dir: &data_dir,
                    lecture_id: &lecture_id,
                    selection: &selection,
                    force,
                    source,
                },
                step,
                event,
            );
            let finished = match &outcome {
                Ok(o) => Finished {
                    lecture_id: lecture_id.clone(),
                    status: "ready",
                    chapters: o.chapters.len(),
                    error: None,
                },
                Err(e) => {
                    eprintln!("[oculus] chapters: {e}");
                    Finished {
                        lecture_id: lecture_id.clone(),
                        status: "error",
                        chapters: 0,
                        error: Some(e.clone()),
                    }
                }
            };
            app.emit(LECTURE_CHAPTERS_EVENT, finished).ok();
        },
    );
    Ok(())
}

/// A lecture's downloaded streams, source 1 first, with what the frame
/// commands name and size them by.
struct Streams {
    title: String,
    /// Echo360's catalogue length.
    duration: u32,
    dir: PathBuf,
    sources: Vec<(crate::sources::echo360::SourceNum, PathBuf)>,
}

async fn downloaded_streams(lecture_id: &str) -> Result<Streams, String> {
    let pool = crate::db::store::open_pool().await?;
    let row: Option<(String, i64, Option<String>, Option<String>)> = sqlx::query_as(
        "SELECT title, duration_seconds, video_path, video2_path FROM lectures WHERE id = ?1",
    )
    .bind(lecture_id)
    .fetch_optional(&pool)
    .await
    .map_err(|e| e.to_string())?;
    let (title, duration, first, second) = row.ok_or_else(|| format!("no lecture {lecture_id}"))?;

    let dir = crate::sources::echo360::lecture_dir(&crate::library::paths::data_dir(), lecture_id);
    // The column, else the stream's own path on disk (as `detect` does).
    let sources = [(1, first), (2, second)]
        .into_iter()
        .map(|(n, column)| {
            let path = column
                .map(PathBuf::from)
                .unwrap_or_else(|| crate::sources::echo360::source_path(&dir, n));
            (n, path)
        })
        .filter(|(_, path)| path.exists())
        .collect();
    Ok(Streams {
        title,
        duration: u32::try_from(duration).unwrap_or(0),
        dir,
        sources,
    })
}

/// The Up Next card's thumbnail (`docs/viewers.md`): one frame a quarter
/// of the way in ([`thumbnail_pick`]), probed like a chapter's, cached at
/// `lectures/<id>/thumb.jpg`. Returns its path for the asset protocol, or
/// `None` while the lecture is not downloaded.
#[tauri::command]
pub async fn lecture_thumbnail(lecture_id: String) -> Result<Option<String>, String> {
    let Streams {
        dir,
        duration,
        sources,
        ..
    } = downloaded_streams(&lecture_id).await?;
    let Some((video, second)) = thumbnail_pick(&sources, duration) else {
        return Ok(None);
    };
    let out = dir.join("thumb.jpg");
    if out.exists() {
        return Ok(Some(out.to_string_lossy().into_owned()));
    }
    let ffmpeg = crate::sources::echo360::find_ffmpeg(None)
        .ok_or("no ffmpeg found — install it, or run `bun run ffmpeg`")?;
    crate::runtime::blocking::run(move || {
        // Written aside and renamed, so a card never loads half a JPEG.
        std::fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
        let part = dir.join("thumb.part.jpg");
        grab_frame(&ffmpeg, &video, second, THUMB_WIDTH, &part)?;
        std::fs::rename(&part, &out).map_err(|e| format!("{}: {e}", out.display()))?;
        Ok(Some(out.to_string_lossy().into_owned()))
    })
    .await
}

/// One stream's frame of the moment a dock message carries.
#[derive(serde::Serialize, Clone)]
pub struct MomentFrame {
    pub source: crate::sources::echo360::SourceNum,
    /// Relative to `agents/`, the thread's cwd.
    pub path: String,
}

/// One JPEG per downloaded stream of the playhead's moment, for a message
/// sent from the lecture player's dock (`docs/harness.md`).
///
/// Every source is grabbed, not the one on screen: either stream may be the
/// one with the teaching on it. Returns paths the agent reads
/// (`../lectures/<id>/frames/live/<seconds>-source<n>.jpg`), in `live/` so
/// they never collide with a chaptering run's. A stream that fails to decode
/// is skipped; only no frame at all is an error.
#[tauri::command]
pub async fn lecture_grab_frames(
    lecture_id: String,
    seconds: u32,
) -> Result<Vec<MomentFrame>, String> {
    let Streams {
        title,
        dir,
        sources,
        ..
    } = downloaded_streams(&lecture_id).await?;
    if sources.is_empty() {
        return Err(format!(
            "{title} is not downloaded — `oculus run -l --videos` fetches it"
        ));
    }
    let ffmpeg = crate::sources::echo360::find_ffmpeg(None)
        .ok_or("no ffmpeg found — install it, or run `bun run ffmpeg`")?;

    let out_dir = dir.join("frames").join("live");
    std::fs::create_dir_all(&out_dir).map_err(|e| format!("{}: {e}", out_dir.display()))?;
    let grabbed: Vec<crate::sources::echo360::SourceNum> = tokio::task::spawn_blocking(move || {
        sources
            .into_iter()
            .filter_map(|(n, video)| {
                let out = out_dir.join(format!("{seconds}-source{n}.jpg"));
                match grab_frame(&ffmpeg, &video, seconds, LIVE_GRAB_WIDTH, &out) {
                    Ok(()) => Some(n),
                    Err(e) => {
                        eprintln!("[oculus] frame: source {n} at {seconds}s: {e}");
                        None
                    }
                }
            })
            .collect()
    })
    .await
    .map_err(|e| e.to_string())?;
    if grabbed.is_empty() {
        return Err(format!("no frame of {title} at {seconds}s"));
    }
    Ok(grabbed
        .into_iter()
        .map(|source| MomentFrame {
            source,
            path: format!("../lectures/{lecture_id}/frames/live/{seconds}-source{source}.jpg"),
        })
        .collect())
}

/// Startup: clear `running` left by a killed run.
pub fn reconcile(_app: &AppHandle) {
    reconcile_status("chapters", |pool| async move {
        crate::db::store::reconcile_chapter_status(&pool).await
    });
}
