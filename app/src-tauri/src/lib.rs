pub mod agent;
pub mod agents;
mod auth;
pub mod browser;
pub mod calendar;
pub mod canvas;
pub mod chapters;
pub mod echo360;
pub mod ed;
mod files;
pub mod harness;
mod ipc;
pub mod keepalive;
mod lectures;
pub mod llm;
pub mod md;
pub mod menu;
pub mod okta;
mod media;
pub mod mineru;
pub mod paths;
pub mod projects;
pub mod recap;
pub mod retrieval;
mod scrape;
pub mod store;
pub mod sync;
pub mod terms;
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
        // ⌘T / ⌘W reach the app as menu events, not key events — see menu.rs.
        .menu(menu::build)
        .on_menu_event(menu::handle)
        .manage(AuthState(Arc::new(Mutex::new(false))))
        .manage(SubjectsState(Arc::new(Mutex::new(vec![]))))
        .manage(Echo360Cache(Arc::new(Mutex::new(std::collections::HashMap::new()))))
        .manage(lectures::DownloadCancels::default())
        .manage(SidecarProcess(Arc::new(Mutex::new(None))))
        .manage(ScrapeCancel::default())
        .manage(agent::ChatCancel::default())
        .manage(browser::BrowserState::default())
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

            // ── In-app browser ──────────────────────────────────────────
            // Hands WebKit the Canvas session before anything can be
            // clicked, so the first Canvas page opened is already signed in,
            // and hooks the main window's resize so pages follow it (see
            // src/browser.rs).
            browser::init(app.handle());

            // ── CLI agents (Claude Code / Codex bridges) ────────────────
            app.manage(harness::app::init(app.handle()));
            harness::app::reconcile(app.handle());
            // A chaptering run killed mid-turn leaves `running` on the
            // lecture row; nothing else will ever clear it.
            chapters::app::reconcile(app.handle());
            // Recap windows commit as they finish, but an interrupted run's
            // `running` marker still needs the same startup repair.
            recap::app::reconcile(app.handle());
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
                            // RETIRED FEATURE, LIVE MIGRATION. Automations and
                            // the Inbox were removed; migrations 18, 20 and 21
                            // stay because they already ran on every existing
                            // database, and because 18 is also where
                            // `sync_schedules` was dropped. The three tables
                            // are created and then left alone — nothing reads
                            // or writes them. Reinstating the feature needs no
                            // new migration; deleting these would need one.
                            //
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
                            // the next sync. A deadline Oculus derives, or a
                            // reminder the user adds, is not Canvas's to
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
                        tauri_plugin_sql::Migration {
                            version: 23,
                            description: "lectures: the second Echo360 source",
                            // A capture is one lesson with up to two streams —
                            // the Presenter screen and the room camera — behind
                            // one media id, so they are columns on the lecture
                            // rather than rows of their own.
                            //
                            // `has_source2` is what Echo360 offers, filled by
                            // the syllabus; `video2_path` is what is actually
                            // on disk. They are separate because the player
                            // needs both answers: whether to show the source
                            // control at all, and whether to offer a download.
                            //
                            // Existing rows default to no second source and
                            // pick the truth up on the next lecture sync;
                            // nothing already downloaded is invalidated, since
                            // source 1 keeps the `source1.mp4` name it had.
                            sql: r#"
ALTER TABLE lectures ADD COLUMN video2_path TEXT;
ALTER TABLE lectures ADD COLUMN has_source2 INTEGER NOT NULL DEFAULT 0;
                        "#,
                            kind: tauri_plugin_sql::MigrationKind::Up,
                        },
                        tauri_plugin_sql::Migration {
                            version: 24,
                            description: "harness: CLI-agent threads and their timeline",
                            // A thread is one conversation with one provider
                            // CLI (`claude` or `codex`); `provider_session_id`
                            // is that CLI's own id for it, which is what a
                            // restart resumes with. Items are the folded
                            // timeline — user, assistant, thinking, tool,
                            // error — written by Rust as the events arrive
                            // (src/harness/store.rs). `ref_id` is the
                            // provider's tool-call id, so the result can find
                            // the row its call made; `meta` is JSON the kind
                            // decides (a tool's kind, input, ok, output).
                            //
                            // `usage` is the last usage report, JSON, kept on
                            // the thread rather than as rows: the composer
                            // shows one number, not a history.
                            sql: r#"
CREATE TABLE IF NOT EXISTS harness_threads (
    id                  INTEGER PRIMARY KEY AUTOINCREMENT,
    provider            TEXT    NOT NULL,
    provider_session_id TEXT,
    model               TEXT,
    title               TEXT,
    status              TEXT    NOT NULL DEFAULT 'idle',
    usage               TEXT,
    created_at          TEXT    NOT NULL DEFAULT (datetime('now')),
    updated_at          TEXT    NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE IF NOT EXISTS harness_items (
    id         INTEGER PRIMARY KEY AUTOINCREMENT,
    thread_id  INTEGER NOT NULL REFERENCES harness_threads(id) ON DELETE CASCADE,
    kind       TEXT    NOT NULL,
    ref_id     TEXT,
    content    TEXT,
    meta       TEXT,
    created_at TEXT    NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_harness_items_thread ON harness_items(thread_id, id);
                        "#,
                            kind: tauri_plugin_sql::MigrationKind::Up,
                        },
                        tauri_plugin_sql::Migration {
                            version: 25,
                            description: "harness: the subject a thread is scoped to",
                            // NULL is the general thread — the whole library.
                            // A code is not stored: the join to `subjects`
                            // keeps a renamed subject's folder name right.
                            sql: r#"
ALTER TABLE harness_threads ADD COLUMN subject_id INTEGER REFERENCES subjects(id) ON DELETE SET NULL;
                        "#,
                            kind: tauri_plugin_sql::MigrationKind::Up,
                        },
                        tauri_plugin_sql::Migration {
                            version: 26,
                            description: "harness: whether the model has named the thread",
                            // A thread is born titled with the first line of
                            // its first message; neither CLI announces a name
                            // over its protocol, so a real one costs a turn
                            // (`Harness::name_thread`). This flag is claimed
                            // before that turn runs, so two completed turns
                            // arriving together cannot both pay for it.
                            sql: r#"
ALTER TABLE harness_threads ADD COLUMN title_generated INTEGER NOT NULL DEFAULT 0;
                        "#,
                            kind: tauri_plugin_sql::MigrationKind::Up,
                        },
                        tauri_plugin_sql::Migration {
                            version: 27,
                            description: "projects: per-subject boards, tasks and subtasks",
                            // A project is a piece of work you are doing — an
                            // assignment, a revision plan — broken into tasks
                            // and one level of subtask, and read back as a
                            // board, a table, a backlog or a timeline.
                            //
                            // `subject_id` is nullable and clears rather than
                            // cascades, for the same reason as `local_events`
                            // (migration 22) and `harness_threads` (25): this
                            // is the user's own planning, and dropping a
                            // course must not take it away. NULL is
                            // "Personal". The Canvas-owned tables cascade
                            // because their rows are the course's; these are
                            // not.
                            //
                            // `columns` is a JSON array of {id, name, kind}
                            // on the project rather than a table of its own.
                            // Columns are renamed per project and their shape
                            // is still moving, which is the same call
                            // `automations.graph` made in migration 18: JSON
                            // for the part that keeps changing, real columns
                            // for the part that gets queried.
                            //
                            // `position` is REAL so dragging a card writes one
                            // row instead of renumbering a column — the new
                            // position is the midpoint between its neighbours.
                            //
                            // Project → tasks *is* ON DELETE CASCADE: a
                            // project genuinely owns its tasks, where a
                            // subject merely scopes them.
                            //
                            // `source` ('manual' | 'agent') because the chat
                            // agent will write tasks through the CLI, and the
                            // board should be able to say which ones you did
                            // not write.
                            sql: r#"
CREATE TABLE IF NOT EXISTS projects (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    subject_id  INTEGER REFERENCES subjects(id) ON DELETE SET NULL,
    name        TEXT    NOT NULL,
    brief       TEXT,
    status      TEXT    NOT NULL DEFAULT 'active',
    starts_at   TEXT,
    due_at      TEXT,
    columns     TEXT    NOT NULL,
    position    REAL    NOT NULL DEFAULT 0,
    source      TEXT    NOT NULL DEFAULT 'manual',
    created_at  TEXT    NOT NULL DEFAULT (datetime('now')),
    updated_at  TEXT    NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE IF NOT EXISTS project_tasks (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    project_id  INTEGER NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    parent_id   INTEGER REFERENCES project_tasks(id) ON DELETE CASCADE,
    title       TEXT    NOT NULL,
    body        TEXT,
    column_id   TEXT    NOT NULL,
    position    REAL    NOT NULL,
    starts_at   TEXT,
    due_at      TEXT,
    estimate_minutes INTEGER,
    done_at     TEXT,
    source      TEXT    NOT NULL DEFAULT 'manual',
    created_at  TEXT    NOT NULL DEFAULT (datetime('now')),
    updated_at  TEXT    NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_project_tasks_project ON project_tasks(project_id, column_id, position);
CREATE INDEX IF NOT EXISTS idx_project_tasks_due     ON project_tasks(due_at);
CREATE INDEX IF NOT EXISTS idx_projects_subject      ON projects(subject_id);
                        "#,
                            kind: tauri_plugin_sql::MigrationKind::Up,
                        },
                        tauri_plugin_sql::Migration {
                            version: 28,
                            description: "harness: the provider's handle for each question",
                            // What a rewind has to name. Claude's
                            // `rewind_conversation` takes the uuid of the user
                            // message in its own transcript; Codex's
                            // `thread/revert` takes the id of the turn to
                            // revert before. Both are learned once, when the
                            // turn goes out, and neither CLI will say it again
                            // later — so it is kept on the row.
                            //
                            // NULL on every row written before this migration,
                            // and on a question whose turn never started.
                            // Editing one of those still rewinds the thread
                            // being read; it just cannot rewind the agent.
                            sql: r#"
ALTER TABLE harness_items ADD COLUMN anchor TEXT;
                        "#,
                            kind: tauri_plugin_sql::MigrationKind::Up,
                        },
                        tauri_plugin_sql::Migration {
                            version: 29,
                            description: "lecture chapters: named topic spans, and how they were made",
                            // What a chaptering run leaves behind. Derived
                            // data, like `pages` and `files.parse_status`:
                            // the recording on disk is the source of truth
                            // and a chapter set is regenerable from it, so
                            // these rows cascade with the lecture and nothing
                            // here is the student's own work.
                            //
                            // **No `end_seconds`.** A chapter ends where the
                            // next one begins, and the last at the lecture's
                            // duration — one fact in one column, which is the
                            // lesson migration 27 records about `column_id` /
                            // `position` / `done_at`. A stored end is a
                            // second place for the same fact to be wrong,
                            // and the reader derives it for free.
                            //
                            // `idx` rather than an autoincrement id: the
                            // order *is* the data, the pair is the identity,
                            // and a regenerate replaces the set wholesale.
                            //
                            // `chapter_status` is NULL | running | ready |
                            // error, mirroring `files.parse_status`
                            // (migration 3); only a terminal status stamps
                            // `chaptered_at`. `chapter_error` carries the
                            // failure message because a status column cannot
                            // — the player has to be able to show what went
                            // wrong and offer a retry — and is cleared on
                            // success.
                            sql: r#"
CREATE TABLE IF NOT EXISTS lecture_chapters (
    lecture_id    TEXT    NOT NULL REFERENCES lectures(id) ON DELETE CASCADE,
    idx           INTEGER NOT NULL,
    start_seconds INTEGER NOT NULL,
    title         TEXT    NOT NULL,
    summary       TEXT    NOT NULL,
    PRIMARY KEY (lecture_id, idx)
);

ALTER TABLE lectures ADD COLUMN chapter_status TEXT;
ALTER TABLE lectures ADD COLUMN chaptered_at   TEXT;
ALTER TABLE lectures ADD COLUMN chapter_error  TEXT;
                        "#,
                            kind: tauri_plugin_sql::MigrationKind::Up,
                        },
                        tauri_plugin_sql::Migration {
                            version: 30,
                            description: "harness: the lecture a thread is scoped to",
                            // A conversation held in the lecture player's
                            // dock, about the recording being watched. NULL
                            // is every other thread.
                            //
                            // SET NULL rather than CASCADE, for the reason
                            // `subject_id` (migration 25) clears: the
                            // conversation is the student's own and the
                            // lecture merely scopes it, so deleting a
                            // recording must not take the conversation away.
                            //
                            // Set at thread creation only, like the subject
                            // beside it — both CLIs bind the appended
                            // instructions at session start, so a re-scope
                            // would be a lie until the process was restarted.
                            // The thread's `subject_id` is filled from the
                            // lecture's own row rather than from the
                            // payload, so the two can never disagree.
                            sql: r#"
ALTER TABLE harness_threads ADD COLUMN lecture_id TEXT REFERENCES lectures(id) ON DELETE SET NULL;
                        "#,
                            kind: tauri_plugin_sql::MigrationKind::Up,
                        },
                        tauri_plugin_sql::Migration {
                            version: 31,
                            description: "lecture recap: slide-level notes and job status",
                            // Recap notes are derived from the recording and
                            // transcript, so they cascade with the lecture and
                            // a regenerate replaces them. There is no stored
                            // end: each note lasts until the next starts.
                            //
                            // Windows are deliberately absent from the table.
                            // They are only an execution detail; `idx` is the
                            // durable play order across separately committed
                            // windows, which lets partial work stay visible.
                            sql: r#"
CREATE TABLE IF NOT EXISTS lecture_recap (
    lecture_id    TEXT    NOT NULL REFERENCES lectures(id) ON DELETE CASCADE,
    idx           INTEGER NOT NULL,
    start_seconds INTEGER NOT NULL,
    label         TEXT    NOT NULL,
    body          TEXT    NOT NULL,
    PRIMARY KEY (lecture_id, idx)
);

ALTER TABLE lectures ADD COLUMN recap_status TEXT;
ALTER TABLE lectures ADD COLUMN recapped_at  TEXT;
ALTER TABLE lectures ADD COLUMN recap_error TEXT;
                        "#,
                            kind: tauri_plugin_sql::MigrationKind::Up,
                        },
                        tauri_plugin_sql::Migration {
                            version: 32,
                            description: "lectures: when the recording was last watched",
                            // `progress_seconds` says how far in you got and
                            // nothing said *when*. Ranking "continue where you
                            // left off" needs the when: the only other
                            // orderable column is `synced_at`, which is when
                            // Echo360 was scraped and has nothing to do with
                            // watching.
                            //
                            // Written by the app's player alone —
                            // `updateLectureProgress` / `markLectureComplete`
                            // in `app/src/lib/db.ts`, stamped with
                            // `datetime('now')` so it compares directly with
                            // `files.last_accessed_at`. The CLI never plays a
                            // recording, so `store.rs` has no writer for it.
                            //
                            // Nullable with no default on purpose: NULL means
                            // never watched, and backfilling every
                            // already-watched lecture to "now" would put the
                            // whole catalogue at the top of Continue on first
                            // launch.
                            sql: r#"
ALTER TABLE lectures ADD COLUMN last_watched_at TEXT;
                        "#,
                            kind: tauri_plugin_sql::MigrationKind::Up,
                        },
                        tauri_plugin_sql::Migration {
                            version: 33,
                            description: "projects: custom tags, and the calendar event a project answers to",
                            // Two facts a project's About page needs and
                            // migration 27 had nowhere to put.
                            //
                            // `tags` is a JSON array of strings on the row,
                            // not a `tags` + `project_tags` pair, and for the
                            // same reason `columns` beside it is JSON: this is
                            // the part that keeps changing, and nothing
                            // queries it in SQL. A student has a handful of
                            // projects, so "every tag I have used" is a scan
                            // over tens of rows in the page rather than a
                            // GROUP BY — and the day a tag needs a colour, a
                            // description or a rename that fans out, that is
                            // the day it earns a table. NOT NULL DEFAULT '[]'
                            // so every reader parses the same shape and no
                            // caller has to spell "untagged" twice.
                            //
                            // `event_id` is the calendar's own id for the
                            // event this project answers to — the Canvas
                            // deadline an assignment is submitted against, or
                            // a local row. It holds a `CalEvent.id` as
                            // `app/src/lib/calendar.ts` mints it, so a Canvas
                            // row is its Canvas id and a local one is
                            // `local_<n>`: one column addressing three tables,
                            // because what is pinned is the thing on the grid
                            // rather than a row in any one of them.
                            //
                            // **Deliberately not a foreign key.** A sync
                            // deletes a subject's `calendar_events` rows and
                            // re-inserts them (see `docs/calendar.md`), so a
                            // REFERENCES … ON DELETE SET NULL would drop every
                            // link on the floor halfway through the next sync
                            // — the ids come back identical, but the cascade
                            // would already have fired. The pointer is
                            // resolved live against `loadCalendar()` instead,
                            // and one that no longer resolves draws nothing:
                            // the same call the calendar's task layer makes in
                            // the other direction.
                            sql: r#"
ALTER TABLE projects ADD COLUMN tags TEXT NOT NULL DEFAULT '[]';
ALTER TABLE projects ADD COLUMN event_id TEXT;
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
            lectures::echo360_cancel_download,
            lectures::echo360_delete_video,
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
            mineru::mineru_set_api_key,
            mineru::mineru_has_api_key,
            mineru::mineru_delete_api_key,
            agent::chat_send,
            agent::chat_cancel,
            harness::app::harness_health,
            harness::app::harness_codex_models,
            harness::app::harness_refresh_rate_limits,
            harness::app::harness_send,
            harness::app::harness_edit_resend,
            harness::app::harness_rewind,
            harness::app::harness_queued,
            harness::app::harness_unqueue,
            harness::app::harness_edit_queued,
            harness::app::harness_interrupt,
            harness::app::harness_delete_thread,
            chapters::app::lecture_find_chapters,
            chapters::app::lecture_grab_frame,
            recap::app::lecture_write_recap,
            sidecar::sidecar_health,
            sidecar::sidecar_set_limits,
            storage::storage_report,
            browser::browser_open_url,
            browser::browser_state,
            browser::browser_place,
            browser::browser_set_viewport,
            browser::browser_hide_tab,
            browser::browser_hide,
            browser::browser_navigate,
            browser::browser_history,
            browser::browser_reload,
            browser::browser_close_tab,
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|app_handle, event| {
            // Don't let uvicorn outlive the window.
            if matches!(event, tauri::RunEvent::Exit) {
                sidecar::shutdown(app_handle);
                harness::app::shutdown(app_handle);
            }
        });
}
