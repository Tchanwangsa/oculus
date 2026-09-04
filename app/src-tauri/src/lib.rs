pub mod agent;
mod auth;
pub mod calendar;
pub mod canvas;
pub mod echo360;
pub mod ed;
mod files;
mod ipc;
pub mod keepalive;
mod lectures;
pub mod llm;
pub mod md;
pub mod okta;
mod media;
pub mod mineru;
pub mod paths;
pub mod retrieval;
mod scrape;
pub mod store;
pub mod sync;
pub mod sidecar;
mod storage;
mod subjects;

use std::sync::{Arc, Mutex};
use tauri::{Emitter, Manager};

use auth::{auth_flag_path, saved_session_probe, AuthProbe, AuthState};
use ipc::IpcPort;
use lectures::Echo360Cache;
use sidecar::SidecarProcess;
use scrape::ScrapeCancel;
use subjects::SubjectsState;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .manage(AuthState(Arc::new(Mutex::new(false))))
        .manage(SubjectsState(Arc::new(Mutex::new(vec![]))))
        .manage(Echo360Cache(Arc::new(Mutex::new(std::collections::HashMap::new()))))
        .manage(SidecarProcess(Arc::new(Mutex::new(None))))
        .manage(ScrapeCancel::default())
        .manage(agent::ChatCancel::default())
        .setup(|app| {
            // ── IPC HTTP server ────────────────────────────────────────
            let port = ipc::start_ipc_server(app.handle().clone());
            app.manage(IpcPort(port));

            // ── Media HTTP server ──────────────────────────────────────
            // WebKit won't play <video> from the asset protocol (see
            // media.rs); lecture playback streams from here instead.
            app.manage(media::start_media_server(paths::data_dir()));

            // ── Python parsing sidecar ─────────────────────────────────
            sidecar::spawn(app.handle());
            // Ctrl-C and the SIGTERM `tauri dev` sends on rebuild bypass
            // Tauri's Exit event, so cleanup needs its own path.
            sidecar::install_exit_handlers(app.handle());

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
                        // A dead session is only a sign-out if we cannot
                        // rebuild it ourselves; `try_auto_recover` emits its
                        // own success event when it can.
                        if okta::try_auto_recover(&app_handle) {
                            *mem.lock().unwrap() = true;
                        } else {
                            eprintln!("[oculus] session rejected — reset to disconnected");
                            std::fs::remove_file(auth_flag_path(&app_handle)).ok();
                            *mem.lock().unwrap() = false;
                            app_handle.emit("canvas-auth-expired", "expired").ok();
                        }
                    }
                    AuthProbe::Unreachable(_) => {
                        eprintln!("[oculus] could not verify session — assuming still good");
                    }
                });
            } else {
                eprintln!("[oculus] no auth flag — fresh session");
            }

            // The agent's plist stores an absolute path to the CLI; if the
            // bundle has moved since it was installed, fix it now.
            keepalive::repair_path(app.handle());

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
                    if okta::try_auto_recover(&ka_handle) {
                        eprintln!("[oculus] keep-alive: session renewed automatically");
                        continue;
                    }
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
        .plugin(tauri_plugin_notification::init())
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
                        tauri_plugin_sql::Migration {
                            version: 8,
                            description: "subjects.selected becomes persistent UI state",
                            // Until now `selected` defaulted to 1 and was never
                            // written by the UI, which auto-selected current
                            // subjects only. Normalise once so the persisted
                            // selection matches what the picker showed.
                            sql: r#"UPDATE subjects SET selected = is_current;"#,
                            kind: tauri_plugin_sql::MigrationKind::Up,
                        },
                        tauri_plugin_sql::Migration {
                            version: 9,
                            description: "file recency: first_seen_at + last_accessed_at",
                            // Pre-existing rows keep NULL first_seen_at on
                            // purpose: only files scraped after this ships get
                            // the "new" indicator, and nothing pretends it was
                            // accessed before tracking existed.
                            sql: r#"
ALTER TABLE files ADD COLUMN first_seen_at    TEXT;
ALTER TABLE files ADD COLUMN last_accessed_at TEXT;
                        "#,
                            kind: tauri_plugin_sql::MigrationKind::Up,
                        },
                        tauri_plugin_sql::Migration {
                            version: 10,
                            description: "office rows keyed by original name, not converted PDF",
                            // Older syncs stored Office documents as their
                            // converted PDF ("deck.pptx.pdf"). The library row
                            // is now the original name; the on-disk PDF stays
                            // as a derived artifact the viewer and parser read.
                            // Keeping the row id preserves pages/embeddings.
                            // OR IGNORE + DELETE handles the rare pair where an
                            // unconverted original was also saved (LibreOffice
                            // missing at the time): the original's row wins.
                            sql: r#"
UPDATE OR IGNORE files SET
  relative_path = substr(relative_path, 1, length(relative_path) - 4),
  filename      = substr(filename,      1, length(filename)      - 4),
  file_type     = CASE
    WHEN lower(filename) LIKE '%.pptx.pdf' THEN 'pptx'
    WHEN lower(filename) LIKE '%.docx.pdf' THEN 'docx'
    WHEN lower(filename) LIKE '%.ppt.pdf'  THEN 'ppt'
    ELSE 'doc'
  END
WHERE lower(relative_path) LIKE '%.pptx.pdf'
   OR lower(relative_path) LIKE '%.docx.pdf'
   OR lower(relative_path) LIKE '%.ppt.pdf'
   OR lower(relative_path) LIKE '%.doc.pdf';

DELETE FROM files
WHERE lower(relative_path) LIKE '%.pptx.pdf'
   OR lower(relative_path) LIKE '%.docx.pdf'
   OR lower(relative_path) LIKE '%.ppt.pdf'
   OR lower(relative_path) LIKE '%.doc.pdf';
                        "#,
                            kind: tauri_plugin_sql::MigrationKind::Up,
                        },
                        tauri_plugin_sql::Migration {
                            version: 11,
                            description: "per-run file ledger for sync history",
                            // One row per file a sync run touched. `action` is
                            // what the write actually did on disk: 'new',
                            // 'updated', or 'unchanged'. Runs from before this
                            // table simply have no rows — the history view
                            // shows them without a file breakdown.
                            sql: r#"
CREATE TABLE IF NOT EXISTS sync_run_files (
    id            INTEGER PRIMARY KEY AUTOINCREMENT,
    run_id        INTEGER NOT NULL REFERENCES sync_runs(id) ON DELETE CASCADE,
    subject_id    INTEGER REFERENCES subjects(id) ON DELETE SET NULL,
    relative_path TEXT    NOT NULL,
    action        TEXT    NOT NULL,
    size_bytes    INTEGER,
    timestamp     TEXT    NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_sync_run_files_run ON sync_run_files(run_id);
                        "#,
                            kind: tauri_plugin_sql::MigrationKind::Up,
                        },
                        tauri_plugin_sql::Migration {
                            version: 12,
                            description: "record which subjects each sync run targeted",
                            // JSON array of course codes, written when the run
                            // starts — so even failed/interrupted runs know
                            // what they were for. NULL on runs from before.
                            sql: r#"ALTER TABLE sync_runs ADD COLUMN subject_codes TEXT;"#,
                            kind: tauri_plugin_sql::MigrationKind::Up,
                        },
                        tauri_plugin_sql::Migration {
                            version: 13,
                            description: "run origin + user-defined sync schedules",
                            // `origin` says what kicked a run off: 'manual'
                            // (button/CLI) or 'scheduled'. Schedules fire from
                            // the frontend while the app is open; `anchor_at`
                            // is the reference point for "has this fired for
                            // the current period yet" — creation time at
                            // first, then bumped to each firing.
                            sql: r#"
ALTER TABLE sync_runs ADD COLUMN origin TEXT NOT NULL DEFAULT 'manual';

CREATE TABLE IF NOT EXISTS sync_schedules (
    id               INTEGER PRIMARY KEY AUTOINCREMENT,
    kind             TEXT    NOT NULL,
    time_of_day      TEXT,
    interval_minutes INTEGER,
    enabled          INTEGER NOT NULL DEFAULT 1,
    anchor_at        TEXT    NOT NULL DEFAULT (datetime('now')),
    last_fired_at    TEXT,
    created_at       TEXT    NOT NULL DEFAULT (datetime('now'))
);
                        "#,
                            kind: tauri_plugin_sql::MigrationKind::Up,
                        },
                        tauri_plugin_sql::Migration {
                            version: 14,
                            description: "derive last-synced from sync_runs",
                            // `subjects.last_synced_at` was a second clock
                            // stamped per scraped file, so it drifted from the
                            // run history (interrupted runs still stamped it).
                            // Last-synced is now derived at read time from the
                            // latest completed run whose subject_codes contain
                            // the subject — sync_runs is the only source.
                            sql: r#"ALTER TABLE subjects DROP COLUMN last_synced_at;"#,
                            kind: tauri_plugin_sql::MigrationKind::Up,
                        },
                        tauri_plugin_sql::Migration {
                            version: 15,
                            description: "track when a file's content last changed",
                            // Stamped when a scrape write's byte-compare says
                            // 'new' or 'updated' — unlike scraped_at, which
                            // bumps every run. A file whose content_changed_at
                            // is newer than its last_accessed_at shows the
                            // unseen dot again (module rows included).
                            sql: r#"ALTER TABLE files ADD COLUMN content_changed_at TEXT;"#,
                            kind: tauri_plugin_sql::MigrationKind::Up,
                        },
                        tauri_plugin_sql::Migration {
                            version: 16,
                            description: "llm usage ledger",
                            // One row per model call, written by Rust (`llm.rs`)
                            // right where the spending limit is enforced — the
                            // sum over the current month is the budget check.
                            // Soft refs only (chat_id has no FK): usage history
                            // must survive a library reset (`clearAllFiles`)
                            // and the chats table only arrives in a later
                            // migration. cost_usd is NULL for local providers.
                            sql: r#"
CREATE TABLE IF NOT EXISTS llm_usage (
    id                INTEGER PRIMARY KEY AUTOINCREMENT,
    provider          TEXT    NOT NULL,
    model             TEXT    NOT NULL,
    purpose           TEXT    NOT NULL,
    prompt_tokens     INTEGER NOT NULL DEFAULT 0,
    completion_tokens INTEGER NOT NULL DEFAULT 0,
    cost_usd          REAL,
    chat_id           INTEGER,
    created_at        TEXT    NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_llm_usage_created ON llm_usage(created_at);
                        "#,
                            kind: tauri_plugin_sql::MigrationKind::Up,
                        },
                        tauri_plugin_sql::Migration {
                            version: 17,
                            description: "chat conversations",
                            // Written by Rust (`agent.rs`), not the frontend:
                            // the loop's own tool-call and tool-result turns
                            // are re-read on the next model turn, so the
                            // history has to be authoritative where the loop
                            // runs. `tool_calls`/`citations` are JSON blobs —
                            // the OpenAI message shape round-trips unchanged.
                            sql: r#"
CREATE TABLE IF NOT EXISTS chats (
    id         INTEGER PRIMARY KEY AUTOINCREMENT,
    title      TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE IF NOT EXISTS chat_messages (
    id           INTEGER PRIMARY KEY AUTOINCREMENT,
    chat_id      INTEGER NOT NULL REFERENCES chats(id) ON DELETE CASCADE,
    role         TEXT    NOT NULL,
    content      TEXT,
    tool_calls   TEXT,
    tool_call_id TEXT,
    citations    TEXT,
    model        TEXT,
    created_at   TEXT    NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_chat_messages_chat ON chat_messages(chat_id);
                        "#,
                            kind: tauri_plugin_sql::MigrationKind::Up,
                        },
                        tauri_plugin_sql::Migration {
                            version: 18,
                            description: "automations (schedules generalised) + inbox",
                            // An automation is a small graph — trigger node
                            // plus action nodes joined by links — stored as
                            // JSON so adding a node kind never needs a
                            // migration. Runtime state (enabled, anchor, last
                            // fired) stays in real columns because the
                            // scheduler queries on it. `sync_schedules` rows
                            // migrate into two-node chains; the table goes.
                            //
                            // Inbox rows keep only soft refs to runs and
                            // files: `clearAllFiles` wipes those tables, and
                            // a digest you already read should survive it.
                            sql: r#"
CREATE TABLE IF NOT EXISTS automations (
    id            INTEGER PRIMARY KEY AUTOINCREMENT,
    name          TEXT    NOT NULL,
    graph         TEXT    NOT NULL,
    enabled       INTEGER NOT NULL DEFAULT 1,
    anchor_at     TEXT    NOT NULL DEFAULT (datetime('now')),
    last_fired_at TEXT,
    created_at    TEXT    NOT NULL DEFAULT (datetime('now')),
    updated_at    TEXT    NOT NULL DEFAULT (datetime('now'))
);

INSERT INTO automations (name, graph, enabled, anchor_at, last_fired_at, created_at)
SELECT
    CASE WHEN kind = 'daily'
         THEN 'Daily sync at ' || COALESCE(time_of_day, '')
         ELSE 'Sync every ' || COALESCE(interval_minutes, 0) || ' min' END,
    json_object(
      'nodes', json_array(
        json_object('id', 't1', 'kind', 'trigger.schedule', 'config',
          json_object('scheduleKind', kind, 'timeOfDay', time_of_day,
                      'intervalMinutes', interval_minutes)),
        json_object('id', 'a1', 'kind', 'action.sync', 'config', json_object())
      ),
      'links', json_array(json_array('t1', 'a1'))
    ),
    enabled, anchor_at, last_fired_at, created_at
FROM sync_schedules;

DROP TABLE IF EXISTS sync_schedules;

-- Ships disabled: it spends tokens on every sync, so it is opt-in.
INSERT INTO automations (name, graph, enabled)
VALUES (
  'Summarise new content after a sync',
  json_object(
    'nodes', json_array(
      json_object('id', 't1', 'kind', 'trigger.event', 'config',
        json_object('event', 'sync-complete')),
      json_object('id', 'a1', 'kind', 'action.scrape_digest', 'config', json_object())
    ),
    'links', json_array(json_array('t1', 'a1'))
  ),
  0
);

CREATE TABLE IF NOT EXISTS inbox_items (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    kind        TEXT    NOT NULL,
    title       TEXT    NOT NULL,
    run_id      INTEGER,
    status      TEXT    NOT NULL DEFAULT 'pending',
    read_at     TEXT,
    archived_at TEXT,
    created_at  TEXT    NOT NULL DEFAULT (datetime('now')),
    updated_at  TEXT    NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE IF NOT EXISTS inbox_item_entries (
    id            INTEGER PRIMARY KEY AUTOINCREMENT,
    item_id       INTEGER NOT NULL REFERENCES inbox_items(id) ON DELETE CASCADE,
    subject_id    INTEGER,
    subject_code  TEXT,
    relative_path TEXT    NOT NULL,
    filename      TEXT    NOT NULL,
    action        TEXT    NOT NULL,
    status        TEXT    NOT NULL DEFAULT 'pending',
    summary_md    TEXT,
    updated_at    TEXT    NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_inbox_entries_item ON inbox_item_entries(item_id);
                        "#,
                            kind: tauri_plugin_sql::MigrationKind::Up,
                        },
                        tauri_plugin_sql::Migration {
                            version: 19,
                            description: "calendar: class times + due dates",
                            // One row per dated occurrence, keyed by Canvas's
                            // own context id (`event_123`, `assignment_456`) so
                            // a re-sync updates in place. Canvas expands
                            // repeating classes server-side, so a semester of
                            // lectures is many rows and there is no recurrence
                            // rule stored here — see `app/src-tauri/src/calendar.rs`.
                            //
                            // Deleting a Canvas event has to remove the row, so
                            // the write path replaces a subject's rows wholesale
                            // rather than upserting into a growing set.
                            sql: r#"
CREATE TABLE IF NOT EXISTS calendar_events (
    id          TEXT    PRIMARY KEY,
    subject_id  INTEGER NOT NULL REFERENCES subjects(id) ON DELETE CASCADE,
    kind        TEXT    NOT NULL,
    title       TEXT    NOT NULL,
    start_at    TEXT    NOT NULL,
    end_at      TEXT,
    all_day     INTEGER NOT NULL DEFAULT 0,
    location    TEXT,
    url         TEXT,
    description TEXT,
    synced_at   TEXT    NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_calendar_events_start   ON calendar_events(start_at);
CREATE INDEX IF NOT EXISTS idx_calendar_events_subject ON calendar_events(subject_id);
                        "#,
                            kind: tauri_plugin_sql::MigrationKind::Up,
                        },
                        tauri_plugin_sql::Migration {
                            version: 20,
                            description: "automations: per-trigger firing state",
                            // An automation may now hold several triggers, and
                            // they fire independently — "daily at 09:00" must
                            // still come due on a graph whose other trigger is
                            // an every-30-minutes interval. The row-level
                            // `anchor_at` cannot express that, so per-trigger
                            // anchors live in a JSON map keyed by node id.
                            // Runtime state, not document: the `graph` blob
                            // stays the user's drawing, untouched by firing.
                            sql: r#"
ALTER TABLE automations ADD COLUMN trigger_state TEXT NOT NULL DEFAULT '{}';
                        "#,
                            kind: tauri_plugin_sql::MigrationKind::Up,
                        },
                        tauri_plugin_sql::Migration {
                            version: 21,
                            description: "inbox: the instruction an item was summarised with",
                            // Summarising is no longer one fixed prompt: an
                            // automation wires files into a "Summarise each
                            // file" node and says what to ask of them. An item
                            // whose fill is interrupted by a quit resumes from
                            // its pending rows, so the question has to survive
                            // with them. NULL means the built-in digest wording.
                            sql: r#"
ALTER TABLE inbox_items ADD COLUMN instruction TEXT;
                        "#,
                            kind: tauri_plugin_sql::MigrationKind::Up,
                        },
                        tauri_plugin_sql::Migration {
                            version: 22,
                            description: "calendar: events Oculus writes itself",
                            // `calendar_events` is Canvas's, and every sync
                            // replaces a subject's rows wholesale (see the
                            // migration above and `replaceCalendarEvents`) —
                            // anything Oculus wrote there would be destroyed by
                            // the next sync. A deadline an automation derives,
                            // or a reminder the user adds, is not Canvas's to
                            // delete, so it lives in its own table and the
                            // calendar merges the two on read.
                            //
                            // `subject_id` is nullable and clears rather than
                            // cascades: a personal note need not belong to a
                            // subject, and dropping a course must not silently
                            // take the user's own rows with it.
                            sql: r#"
CREATE TABLE IF NOT EXISTS local_events (
    id         INTEGER PRIMARY KEY AUTOINCREMENT,
    subject_id INTEGER REFERENCES subjects(id) ON DELETE SET NULL,
    kind       TEXT    NOT NULL,
    title      TEXT    NOT NULL,
    start_at   TEXT    NOT NULL,
    end_at     TEXT,
    all_day    INTEGER NOT NULL DEFAULT 0,
    notes      TEXT,
    source     TEXT    NOT NULL DEFAULT 'automation',
    created_at TEXT    NOT NULL DEFAULT (datetime('now'))
);

-- The only read is "every row, in time order" — the page holds the whole set
-- like it does for `calendar_events` — so start_at is the one index earning
-- its keep. No subject index: nothing queries a subject's local rows alone.
CREATE INDEX IF NOT EXISTS idx_local_events_start ON local_events(start_at);
                        "#,
                            kind: tauri_plugin_sql::MigrationKind::Up,
                        },
                    ],
                )
                .build(),
        )
        .invoke_handler(tauri::generate_handler![
            auth::get_auth_status,
            auth::check_canvas_session,
            auth::launch_canvas_auth,
            auth::disconnect_canvas,
            okta::okta_credential_status,
            okta::okta_save_credentials,
            okta::okta_clear_credentials,
            okta::okta_sign_in,
            keepalive::keepalive_status,
            keepalive::keepalive_enable,
            keepalive::keepalive_disable,
            subjects::sync_subjects,
            subjects::get_subjects,
            scrape::scrape_content,
            scrape::cancel_scrape,
            scrape::rescrape_file,
            scrape::parse_file,
            files::read_course_file,
            files::open_course_file,
            files::scan_parsed_files,
            calendar::calendar_sync_events,
            lectures::echo360_sync_lectures,
            lectures::echo360_download_video,
            lectures::echo360_download_transcript,
            lectures::echo360_read_transcript,
            lectures::echo360_clear_transcripts,
            media::media_server_info,
            retrieval::embed_file,
            retrieval::search_pages,
            retrieval::embedding_stats,
            llm::llm_set_api_key,
            llm::llm_has_api_key,
            llm::llm_delete_api_key,
            llm::llm_list_models,
            llm::llm_test_prompt,
            llm::llm_usage_summary,
            llm::llm_summarize,
            llm::llm_generate,
            mineru::mineru_set_api_key,
            mineru::mineru_has_api_key,
            mineru::mineru_delete_api_key,
            agent::chat_send,
            agent::chat_cancel,
            sidecar::sidecar_health,
            sidecar::sidecar_set_limits,
            storage::storage_report,
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
