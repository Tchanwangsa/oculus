mod auth;
mod cors;
mod files;
mod ipc;
mod lectures;
mod scrape;
mod subjects;
mod worker;

use std::sync::{Arc, Mutex};
use tauri::{Emitter, Manager};

use auth::{auth_flag_path, saved_cookie_valid, AuthState};
use ipc::IpcPort;
use lectures::Echo360Cache;
use subjects::SubjectsState;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .manage(AuthState(Arc::new(Mutex::new(false))))
        .manage(SubjectsState(Arc::new(Mutex::new(vec![]))))
        .manage(Echo360Cache(Arc::new(Mutex::new(std::collections::HashMap::new()))))
        .setup(|app| {
            // ── IPC HTTP server ────────────────────────────────────────
            let port = ipc::start_ipc_server(app.handle().clone());
            app.manage(IpcPort(port));

            // ── Hidden worker WebView (hosts scraper/subjects JS) ───────
            worker::ensure_worker_window(app.handle(), port);

            // ── Session restore on startup ──────────────────────────────
            // No WebView dance: we replay the persisted session cookie via a
            // server-side ureq ping. Valid → connected instantly. Invalid →
            // drop the dead cookie + flag (keep the SSO profile so re-login is
            // a tap, not a full sign-in) and tell the UI to reconnect.
            let app_handle = app.handle().clone();
            let flag = auth_flag_path(&app_handle);

            if flag.exists() {
                eprintln!("[oculus] auth flag found — checking persisted cookie");
                let auth_state = app.state::<AuthState>();
                // Optimistic until the async check below corrects it.
                *auth_state.0.lock().unwrap() = true;
                let mem = Arc::clone(&auth_state.0);

                std::thread::spawn(move || {
                    if saved_cookie_valid(&app_handle) {
                        eprintln!("[oculus] persisted session valid");
                        *mem.lock().unwrap() = true;
                        app_handle.emit("canvas-auth-success", "ok").ok();
                    } else {
                        eprintln!("[oculus] persisted session expired — reset to disconnected");
                        std::fs::remove_file(auth_flag_path(&app_handle)).ok();
                        *mem.lock().unwrap() = false;
                        app_handle.emit("canvas-auth-expired", "expired").ok();
                    }
                });
            } else {
                eprintln!("[oculus] no auth flag — fresh session");
            }

            Ok(())
        })
        .plugin(tauri_plugin_opener::init())
        .plugin(
            tauri_plugin_sql::Builder::new()
                .add_migrations(
                    "sqlite:oculus.db",
                    vec![
                        tauri_plugin_sql::Migration {
                            version: 1,
                            description: "initial schema",
                            sql: r#"
CREATE TABLE IF NOT EXISTS subjects (
    id            INTEGER PRIMARY KEY,
    code          TEXT    NOT NULL,
    name          TEXT    NOT NULL,
    term_name     TEXT,
    is_current    INTEGER NOT NULL DEFAULT 0,
    workflow_state TEXT   NOT NULL DEFAULT 'available',
    selected      INTEGER NOT NULL DEFAULT 1,
    last_synced_at TEXT,
    created_at    TEXT    NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE IF NOT EXISTS sync_runs (
    id               INTEGER PRIMARY KEY AUTOINCREMENT,
    started_at       TEXT    NOT NULL DEFAULT (datetime('now')),
    finished_at      TEXT,
    status           TEXT    NOT NULL DEFAULT 'running',
    subjects_synced  INTEGER NOT NULL DEFAULT 0,
    pages_scraped    INTEGER NOT NULL DEFAULT 0,
    error            TEXT
);

CREATE TABLE IF NOT EXISTS sync_log (
    id         INTEGER PRIMARY KEY AUTOINCREMENT,
    run_id     INTEGER REFERENCES sync_runs(id) ON DELETE SET NULL,
    subject_id INTEGER REFERENCES subjects(id)  ON DELETE SET NULL,
    timestamp  TEXT    NOT NULL DEFAULT (datetime('now')),
    level      TEXT    NOT NULL DEFAULT 'info',
    message    TEXT    NOT NULL
);

CREATE TABLE IF NOT EXISTS files (
    id            INTEGER PRIMARY KEY AUTOINCREMENT,
    subject_id    INTEGER NOT NULL REFERENCES subjects(id) ON DELETE CASCADE,
    filename      TEXT    NOT NULL,
    relative_path TEXT    NOT NULL,
    file_type     TEXT    NOT NULL,
    size_bytes    INTEGER,
    scraped_at    TEXT    NOT NULL DEFAULT (datetime('now')),
    UNIQUE(subject_id, relative_path)
);

CREATE TABLE IF NOT EXISTS settings (
    key        TEXT PRIMARY KEY,
    value      TEXT NOT NULL,
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);
                        "#,
                            kind: tauri_plugin_sql::MigrationKind::Up,
                        },
                        tauri_plugin_sql::Migration {
                            version: 2,
                            description:
                                "file metadata: category, source_url, canvas_id, modified_at",
                            sql: r#"
ALTER TABLE files ADD COLUMN category    TEXT;
ALTER TABLE files ADD COLUMN source_url  TEXT;
ALTER TABLE files ADD COLUMN canvas_id   INTEGER;
ALTER TABLE files ADD COLUMN modified_at TEXT;
                        "#,
                            kind: tauri_plugin_sql::MigrationKind::Up,
                        },
                        tauri_plugin_sql::Migration {
                            version: 3,
                            description: "pdf parse status tracking",
                            sql: r#"
ALTER TABLE files ADD COLUMN parse_status TEXT;
ALTER TABLE files ADD COLUMN parsed_at    TEXT;
                        "#,
                            kind: tauri_plugin_sql::MigrationKind::Up,
                        },
                        tauri_plugin_sql::Migration {
                            version: 4,
                            description: "lecture capture",
                            sql: r#"
CREATE TABLE IF NOT EXISTS lectures (
    id                TEXT PRIMARY KEY,
    lesson_id         TEXT UNIQUE NOT NULL,
    subject_id        INTEGER NOT NULL,
    title             TEXT NOT NULL,
    date              TEXT NOT NULL,
    duration_seconds  INTEGER NOT NULL DEFAULT 0,
    video_path        TEXT,
    transcript_path   TEXT,
    progress_seconds  INTEGER NOT NULL DEFAULT 0,
    completed         INTEGER NOT NULL DEFAULT 0,
    synced_at         TEXT NOT NULL DEFAULT (datetime('now'))
);
                        "#,
                            kind: tauri_plugin_sql::MigrationKind::Up,
                        },
                        tauri_plugin_sql::Migration {
                            version: 5,
                            description: "lecture trim offset",
                            sql: r#"
ALTER TABLE lectures ADD COLUMN trim_offset INTEGER NOT NULL DEFAULT 0;
UPDATE lectures SET trim_offset = 14 WHERE video_path IS NOT NULL;
                        "#,
                            kind: tauri_plugin_sql::MigrationKind::Up,
                        },
                    ],
                )
                .build(),
        )
        .invoke_handler(tauri::generate_handler![
            auth::get_auth_status,
            auth::launch_canvas_auth,
            auth::disconnect_canvas,
            auth::open_canvas_devtools,
            subjects::sync_subjects,
            subjects::get_subjects,
            scrape::scrape_content,
            scrape::cancel_scrape,
            scrape::rescrape_file,
            files::read_course_file,
            files::open_course_file,
            files::scan_parsed_files,
            lectures::echo360_sync_lectures,
            lectures::echo360_download_video,
            lectures::echo360_download_transcript,
            lectures::echo360_read_transcript,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
