mod auth;
mod cors;
mod files;
mod ipc;
mod keepalive;
mod lectures;
pub mod retrieval;
mod scrape;
mod sidecar;
mod subjects;
mod worker;

use std::sync::{Arc, Mutex};
use tauri::{Emitter, Manager};

use auth::{auth_flag_path, saved_session_probe, AuthProbe, AuthState};
use ipc::IpcPort;
use lectures::Echo360Cache;
use sidecar::SidecarProcess;
use subjects::SubjectsState;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .manage(AuthState(Arc::new(Mutex::new(false))))
        .manage(SubjectsState(Arc::new(Mutex::new(vec![]))))
        .manage(Echo360Cache(Arc::new(Mutex::new(std::collections::HashMap::new()))))
        .manage(SidecarProcess(Arc::new(Mutex::new(None))))
        .setup(|app| {
            // ── IPC HTTP server ────────────────────────────────────────
            let port = ipc::start_ipc_server(app.handle().clone());
            app.manage(IpcPort(port));

            // ── Python parsing sidecar ─────────────────────────────────
            sidecar::spawn(app.handle());

            // ── Hidden worker WebView (hosts scraper/subjects JS) ───────
            worker::ensure_worker_window(app.handle(), port);

            // ── Cleanup orphaned partial lecture downloads ──────────────
            lectures::cleanup_partial_downloads(app.handle());

            // ── Session restore on startup ──────────────────────────────
            // No WebView dance: we replay the persisted session cookie via a
            // server-side ureq ping. Valid → connected instantly. Rejected →
            // drop the flag (keep the SSO profile so re-login is a tap) and
            // tell the UI to reconnect. Unreachable → stay optimistic; an
            // offline start is not an expired session.
            let app_handle = app.handle().clone();

            if auth_flag_path(&app_handle).exists() {
                eprintln!("[oculus] auth flag found — verifying persisted session");
                let auth_state = app.state::<AuthState>();
                // Optimistic until the async check below corrects it.
                *auth_state.0.lock().unwrap() = true;
                let mem = Arc::clone(&auth_state.0);

                std::thread::spawn(move || match saved_session_probe(&app_handle) {
                    AuthProbe::Valid(_) => {
                        *mem.lock().unwrap() = true;
                        app_handle.emit("canvas-auth-success", "ok").ok();
                    }
                    AuthProbe::Rejected(_) => {
                        eprintln!("[oculus] session rejected — reset to disconnected");
                        std::fs::remove_file(auth_flag_path(&app_handle)).ok();
                        *mem.lock().unwrap() = false;
                        app_handle.emit("canvas-auth-expired", "expired").ok();
                    }
                    AuthProbe::Unreachable(_) => {
                        eprintln!("[oculus] could not verify session — assuming still good");
                    }
                });
            } else {
                eprintln!("[oculus] no auth flag — fresh session");
            }

            // ── In-app keep-alive ───────────────────────────────────────
            // Canvas refreshes the session on each request, so a periodic ping
            // holds it open while Oculus is running. The LaunchAgent in
            // `keepalive.rs` covers the app being closed.
            let ka_handle = app.handle().clone();
            std::thread::spawn(move || loop {
                std::thread::sleep(std::time::Duration::from_secs(6 * 3600));
                if !auth_flag_path(&ka_handle).exists() {
                    continue;
                }
                if let AuthProbe::Rejected(_) = saved_session_probe(&ka_handle) {
                    eprintln!("[oculus] keep-alive: session expired");
                    std::fs::remove_file(auth_flag_path(&ka_handle)).ok();
                    if let Some(state) = ka_handle.try_state::<AuthState>() {
                        *state.0.lock().unwrap() = false;
                    }
                    ka_handle.emit("canvas-auth-expired", "expired").ok();
                }
            });

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
                            description: "lecture trim offset — added then removed",
                            sql: r#"ALTER TABLE lectures ADD COLUMN trim_offset INTEGER NOT NULL DEFAULT 0;"#,
                            kind: tauri_plugin_sql::MigrationKind::Up,
                        },
                        tauri_plugin_sql::Migration {
                            version: 6,
                            description: "drop trim_offset column",
                            // SQLite has no `DROP COLUMN IF EXISTS` — it parses
                            // as a syntax error, which aborted this migration
                            // and every one after it. Migration 5 always adds
                            // the column, so a plain DROP is safe here.
                            sql: r#"ALTER TABLE lectures DROP COLUMN trim_offset;"#,
                            kind: tauri_plugin_sql::MigrationKind::Up,
                        },
                        tauri_plugin_sql::Migration {
                            version: 7,
                            description: "page-level markdown + retrieval embeddings",
                            sql: r#"
-- One row per PDF page. `markdown` is what the LLM reads; `embedding` is what
-- the retriever ranks on, computed from the rendered page image. The two are
-- two representations of the same page joined on (file_id, page_no) — that key
-- is what lets a vector hit resolve to text and to a deep link.
CREATE TABLE IF NOT EXISTS pages (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    file_id     INTEGER NOT NULL REFERENCES files(id) ON DELETE CASCADE,
    page_no     INTEGER NOT NULL,
    markdown    TEXT    NOT NULL DEFAULT '',
    -- Unit-length float16, little-endian. Stored normalised so ranking is a
    -- plain dot product.
    embedding   BLOB,
    embed_model TEXT,
    embed_dim   INTEGER,
    embedded_at TEXT,
    UNIQUE(file_id, page_no)
);

CREATE INDEX IF NOT EXISTS idx_pages_file ON pages(file_id);

ALTER TABLE files ADD COLUMN embed_status TEXT;
ALTER TABLE files ADD COLUMN embedded_at  TEXT;
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
            keepalive::keepalive_status,
            keepalive::keepalive_enable,
            keepalive::keepalive_disable,
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
            lectures::echo360_clear_transcripts,
            retrieval::embed_file,
            retrieval::search_pages,
            retrieval::embedding_stats,
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|app_handle, event| {
            // Don't let uvicorn outlive the window.
            if matches!(event, tauri::RunEvent::Exit) {
                sidecar::shutdown(app_handle);
            }
        });
}
