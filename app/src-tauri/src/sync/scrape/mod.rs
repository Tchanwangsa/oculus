//! The app's entry point into the scrape engine: it runs on a plain thread and
//! reports through [`AppReporter`] as Tauri events.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use tauri::{AppHandle, Emitter};

use crate::sources::canvas::Canvas;
use crate::sync::{
    Engine, FileEvent, FileFailed, FileStart, Progress, Reporter, Subject, SyncOptions,
};

pub(crate) mod embeds;
pub(crate) mod videos;
pub use videos::VideoCancels;

#[derive(serde::Deserialize)]
pub struct ScrapeSubject {
    pub id: i64,
    pub code: String,
}

/// Set by `cancel_scrape`, read by the running engine between items.
pub struct ScrapeCancel(pub Arc<AtomicBool>);

impl Default for ScrapeCancel {
    fn default() -> Self {
        ScrapeCancel(Arc::new(AtomicBool::new(false)))
    }
}

static SCRAPING: AtomicBool = AtomicBool::new(false);

struct ScrapeClaim;
impl ScrapeClaim {
    fn take() -> Result<Self, String> {
        SCRAPING
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .map(|_| Self)
            .map_err(|_| "A sync is already running".into())
    }
}
impl Drop for ScrapeClaim {
    fn drop(&mut self) {
        SCRAPING.store(false, Ordering::SeqCst);
    }
}

#[tauri::command]
pub fn cancel_scrape(cancel: tauri::State<ScrapeCancel>) -> Result<(), String> {
    cancel.0.store(true, Ordering::SeqCst);
    elog!("[oculus] cancel_scrape: signalled");
    Ok(())
}

#[tauri::command]
pub async fn scrape_content(
    app: AppHandle,
    subjects: Vec<ScrapeSubject>,
    options: Option<SyncOptions>,
    cancel: tauri::State<'_, ScrapeCancel>,
) -> Result<(), String> {
    let claim = ScrapeClaim::take()?;
    if subjects.is_empty() {
        return Err("No subjects selected.".to_string());
    }

    // A missing session is not checked for: oculus-keyd signs in on demand, and
    // a run that cannot says so through `canvas-auth-expired`.
    if let Err(e) = Canvas::open(&crate::library::paths::data_dir()).check_keyd() {
        return Err(e.to_string());
    }

    let data_dir = crate::library::paths::data_dir();
    let targets: Vec<Subject> = subjects
        .into_iter()
        .map(|s| Subject {
            id: s.id,
            code: s.code,
        })
        .collect();

    let flag = Arc::clone(&cancel.0);
    flag.store(false, Ordering::SeqCst);

    elog!("[oculus] scrape: {} subject(s)", targets.len());

    // Returns immediately; the frontend follows the events.
    std::thread::spawn(move || {
        let _claim = claim;
        let reporter = AppReporter {
            app: app.clone(),
            cancel: Arc::clone(&flag),
        };
        let engine =
            Engine::new(&data_dir, Box::new(reporter)).with_options(options.unwrap_or_default());
        let count = engine.scrape(&targets);
        let cancelled = flag.load(Ordering::SeqCst);

        elog!("[oculus] scrape finished: {count} subject(s), cancelled={cancelled}");
        app.emit(
            "scrape-complete",
            serde_json::json!({ "count": count, "cancelled": cancelled }),
        )
        .ok();
    });

    Ok(())
}

struct AppReporter {
    app: AppHandle,
    cancel: Arc<AtomicBool>,
}

impl Reporter for AppReporter {
    fn progress(&self, p: &Progress) {
        self.app.emit("scrape-progress", p).ok();
    }

    fn file_start(&self, f: &FileStart) {
        self.app.emit("scrape-file-start", f).ok();
    }

    fn file(&self, f: &FileEvent) {
        elog!(
            "[oculus] wrote {} ({} bytes)",
            f.relative_path,
            f.size_bytes
        );
        self.app.emit("scrape-file", f).ok();
    }

    fn file_failed(&self, f: &FileFailed) {
        elog!("[oculus] download failed: {}: {}", f.relative_path, f.error);
        self.app.emit("scrape-file-failed", f).ok();
    }

    fn log(&self, level: &str, course: &str, message: &str) {
        self.app
            .emit(
                "scrape-log",
                serde_json::json!({ "level": level, "course": course, "message": message }),
            )
            .ok();
    }

    fn cancelled(&self) -> bool {
        self.cancel.load(Ordering::SeqCst)
    }

    fn canvas_expired(&self, message: &str) {
        self.app.emit("canvas-auth-expired", message).ok();
    }
}

/// Parse one already-downloaded PDF (a no-op if already parsed), or convert a
/// spreadsheet to its text again. Fire-and-forget: progress arrives as
/// `parse-status` events. A refusal is also an `error` status, or a sweep
/// would re-kick the row unseen.
#[tauri::command]
pub fn parse_file(
    subject_id: i64,
    subject_code: String,
    relative_path: String,
) -> Result<(), String> {
    let data_dir = crate::library::paths::data_dir();
    let refuse = |detail: String| {
        // `Io`, as `sync::run_parse` reports the same checks.
        let error = crate::parse::ParseError::Io(detail.clone());
        crate::parse::events::failed(&relative_path, subject_id, &error);
        detail
    };
    // A spreadsheet is converted to text in-process, never parsed.
    if crate::library::paths::is_sheet(&relative_path) {
        if !data_dir.join(&relative_path).is_file() {
            return Err(refuse(format!("not on disk: {relative_path}")));
        }
        std::thread::spawn(move || {
            match crate::pages::sheets::index(&data_dir, &relative_path, subject_id) {
                Ok(pages) => {
                    elog!("[oculus] parse_file {relative_path}: {pages} sheet(s) as text")
                }
                Err(e) => elog!("[oculus] parse_file {relative_path}: {e}"),
            }
        });
        return Ok(());
    }
    // For Office files this is the derived sibling PDF.
    let pdf_rel = crate::library::paths::doc_pdf_rel(&relative_path)
        .ok_or_else(|| refuse(format!("{relative_path}: not a parseable file")))?;
    if !data_dir.join(&pdf_rel).is_file() {
        return Err(refuse(format!("not on disk: {pdf_rel}")));
    }
    // Unused, but the frontend sends it.
    let _ = subject_code;
    std::thread::spawn(move || {
        match crate::sync::parse_pdf(&data_dir, &relative_path, subject_id) {
            Ok(summary) => elog!("[oculus] parse_file {relative_path}: {summary}"),
            Err(e) => elog!("[oculus] parse_file {relative_path}: {e}"),
        }
    });
    Ok(())
}

/// Skip one file's parse, or lift the skip. A skip marks the PDF in
/// `parse::Skips`, which ends any parse of it in flight without writing
/// artifacts, and reports `skipped` at once whether or not one was running.
/// Lifting it emits nothing; the frontend then calls `parse_file`.
#[tauri::command]
pub fn parse_skip(relative_path: String, subject_id: i64, skip: bool) -> Result<(), String> {
    let key = crate::sync::parse_key(&crate::library::paths::data_dir(), &relative_path);
    if skip {
        crate::parse::Skips::shared().mark(&key);
        crate::parse::events::skipped(&relative_path, subject_id);
    } else {
        crate::parse::Skips::shared().clear(&key);
    }
    Ok(())
}

/// Await the same native work for a confirmed job card, so its durable result
/// reflects completion rather than the point at which a worker was spawned.
pub(crate) async fn confirmed(
    app: AppHandle,
    targets: Vec<Subject>,
    lectures: bool,
) -> Result<(), String> {
    let _claim = ScrapeClaim::take()?;
    if lectures {
        for target in targets {
            use tauri::Manager;
            let lectures = crate::lectures::commands::echo360_sync_lectures(
                app.state::<crate::lectures::Echo360Cache>(),
                target.id,
            )
            .await?;
            let pool = crate::db::store::open_pool().await?;
            crate::db::store::upsert_lectures(&pool, target.id, &lectures).await?;
            app.emit("lectures-changed", ()).ok();
        }
        return Ok(());
    }
    use tauri::Manager;
    let flag = Arc::clone(&app.state::<ScrapeCancel>().0);
    flag.store(false, Ordering::SeqCst);
    let pool = crate::db::store::open_pool().await?;
    let codes: Vec<String> = targets.iter().map(|s| s.code.clone()).collect();
    let run_id = crate::db::store::start_run(&pool, &codes).await?;
    app.emit("scrape-started",serde_json::json!({"runId":run_id,"subjects":targets.iter().map(|s|serde_json::json!({"id":s.id,"code":s.code})).collect::<Vec<_>>()})).ok();
    let completion = app.clone();
    let outcome = crate::runtime::blocking::run(move || {
        let reporter = AppReporter {
            app: app.clone(),
            cancel: flag.clone(),
        };
        let engine = Engine::new(&crate::library::paths::data_dir(), Box::new(reporter));
        engine.canvas.check_keyd().map_err(|e| e.to_string())?;
        let count = engine.scrape(&targets);
        let cancelled = flag.load(Ordering::SeqCst);
        app.emit(
            "scrape-complete",
            serde_json::json!({"count":count,"cancelled":cancelled}),
        )
        .ok();
        if cancelled {
            return Err("Sync cancelled".into());
        }
        if let Some(error) = engine.canvas.expired() {
            return Err(error.to_string());
        }
        if count != targets.len() {
            return Err(format!("Synced {count} of {} subjects", targets.len()));
        }
        Ok(())
    })
    .await;
    if let Err(error) = &outcome {
        completion.emit("scrape-error", error).ok();
        crate::db::store::finish_run(&pool, run_id, "failed", 0, Some(error)).await?;
    } else {
        crate::db::store::finish_run(&pool, run_id, "completed", codes.len(), None).await?;
    }
    outcome
}

#[cfg(test)]
mod claim_tests {
    use super::*;
    #[test]
    fn one_sync_owns_the_cancel_flag_until_it_finishes() {
        let first = ScrapeClaim::take().unwrap();
        assert!(ScrapeClaim::take().is_err());
        drop(first);
        assert!(ScrapeClaim::take().is_ok());
    }
}
