//! The app's entry point into the scrape engine: it runs on a plain thread and
//! reports through [`AppReporter`] as Tauri events.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use tauri::{AppHandle, Emitter};

use crate::sync::{Engine, FileEvent, FileStart, Progress, Reporter, Subject, SyncOptions};

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

#[tauri::command]
pub fn cancel_scrape(cancel: tauri::State<ScrapeCancel>) -> Result<(), String> {
    cancel.0.store(true, Ordering::SeqCst);
    eprintln!("[oculus] cancel_scrape: signalled");
    Ok(())
}

#[tauri::command]
pub async fn scrape_content(
    app: AppHandle,
    subjects: Vec<ScrapeSubject>,
    options: Option<SyncOptions>,
    cancel: tauri::State<'_, ScrapeCancel>,
) -> Result<(), String> {
    if subjects.is_empty() {
        return Err("No subjects selected.".to_string());
    }

    if crate::files::proxy_cookie(&app).is_empty() {
        app.emit("canvas-auth-expired", "not-authenticated").ok();
        return Err("Not authenticated. Connect to Canvas first.".to_string());
    }

    let data_dir = crate::paths::data_dir();
    let targets: Vec<Subject> = subjects
        .into_iter()
        .map(|s| Subject { id: s.id, code: s.code })
        .collect();

    let flag = Arc::clone(&cancel.0);
    flag.store(false, Ordering::SeqCst);

    eprintln!("[oculus] scrape: {} subject(s)", targets.len());

    // Returns immediately; the frontend follows the events.
    std::thread::spawn(move || {
        let reporter = AppReporter {
            app: app.clone(),
            cancel: Arc::clone(&flag),
        };
        let engine = Engine::new(&data_dir, Box::new(reporter))
            .with_options(options.unwrap_or_default());
        let count = engine.scrape(&targets);
        let cancelled = flag.load(Ordering::SeqCst);

        eprintln!("[oculus] scrape finished: {count} subject(s), cancelled={cancelled}");
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
        eprintln!("[oculus] wrote {} ({} bytes)", f.relative_path, f.size_bytes);
        self.app.emit("scrape-file", f).ok();
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
}

/// Parse one already-downloaded PDF (a no-op if already parsed).
/// Fire-and-forget: progress arrives as `parse-status` events.
#[tauri::command]
pub fn parse_file(
    subject_id: i64,
    subject_code: String,
    relative_path: String,
) -> Result<(), String> {
    let data_dir = crate::paths::data_dir();
    // For Office files this is the derived sibling PDF.
    let pdf_rel = crate::paths::doc_pdf_rel(&relative_path)
        .ok_or_else(|| format!("{relative_path}: not a parseable file"))?;
    if !data_dir.join(&pdf_rel).is_file() {
        return Err(format!("not on disk: {pdf_rel}"));
    }
    // Unused, but the frontend sends it.
    let _ = subject_code;
    std::thread::spawn(move || {
        match crate::sync::parse_pdf(&data_dir, &relative_path, subject_id) {
            Ok(summary) => eprintln!("[oculus] parse_file {relative_path}: {summary}"),
            Err(e) => eprintln!("[oculus] parse_file {relative_path}: {e}"),
        }
    });
    Ok(())
}

/// Re-download one file on demand, e.g. after the user deletes a bad copy.
#[tauri::command]
pub fn rescrape_file(
    app: AppHandle,
    subject_id: i64,
    subject_code: String,
    canvas_id: i64,
) -> Result<String, String> {
    if crate::files::proxy_cookie(&app).is_empty() {
        return Err("Not authenticated — connect to Canvas first.".to_string());
    }
    let data_dir = crate::paths::data_dir();
    let engine = Engine::new(
        &data_dir,
        Box::new(AppReporter {
            app: app.clone(),
            cancel: Arc::new(AtomicBool::new(false)),
        }),
    );

    engine
        .refetch_file(&Subject { id: subject_id, code: subject_code }, canvas_id)?
        .ok_or_else(|| "Canvas would not serve that file (locked, or not a supported type)".to_string())
}
