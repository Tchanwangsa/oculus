//! The SQLite schema: the ordered migrations `tauri-plugin-sql` applies to
//! `oculus.db`. The plugin checksums each one's SQL, so an applied migration is
//! never edited — not even its whitespace, which is why the raw-string closers
//! keep their indent — and a retired feature's migrations stay.

use tauri_plugin_sql::{Migration, MigrationKind};

pub fn all() -> Vec<Migration> {
    vec![
        Migration {
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
            kind: MigrationKind::Up,
        },
        Migration {
            version: 2,
            description:
                "file metadata: category, source_url, canvas_id, modified_at",
            sql: r#"
ALTER TABLE files ADD COLUMN category    TEXT;
ALTER TABLE files ADD COLUMN source_url  TEXT;
ALTER TABLE files ADD COLUMN canvas_id   INTEGER;
ALTER TABLE files ADD COLUMN modified_at TEXT;
                        "#,
            kind: MigrationKind::Up,
        },
        Migration {
            version: 3,
            description: "pdf parse status tracking",
            sql: r#"
ALTER TABLE files ADD COLUMN parse_status TEXT;
ALTER TABLE files ADD COLUMN parsed_at    TEXT;
                        "#,
            kind: MigrationKind::Up,
        },
        Migration {
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
            kind: MigrationKind::Up,
        },
        Migration {
            version: 5,
            description: "lecture trim offset — added then removed",
            sql: r#"ALTER TABLE lectures ADD COLUMN trim_offset INTEGER NOT NULL DEFAULT 0;"#,
            kind: MigrationKind::Up,
        },
        Migration {
            version: 6,
            description: "drop trim_offset column",
            // SQLite has no `DROP COLUMN IF EXISTS` (a syntax error that aborts every
            // later migration); 5 always adds the column, so a plain DROP is safe.
            sql: r#"ALTER TABLE lectures DROP COLUMN trim_offset;"#,
            kind: MigrationKind::Up,
        },
        Migration {
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
            kind: MigrationKind::Up,
        },
        Migration {
            version: 8,
            description: "subjects.selected becomes persistent UI state",
            // The UI starts persisting the selection; seed it from what it showed.
            sql: r#"UPDATE subjects SET selected = is_current;"#,
            kind: MigrationKind::Up,
        },
        Migration {
            version: 9,
            description: "file recency: first_seen_at + last_accessed_at",
            // Pre-existing rows stay NULL, so only files scraped from now on show as new.
            sql: r#"
ALTER TABLE files ADD COLUMN first_seen_at    TEXT;
ALTER TABLE files ADD COLUMN last_accessed_at TEXT;
                        "#,
            kind: MigrationKind::Up,
        },
        Migration {
            version: 10,
            description: "office rows keyed by original name, not converted PDF",
            // Office rows were stored as their converted PDF ("deck.pptx.pdf"); rename
            // in place to keep pages/embeddings. Where both exist, the original's row wins.
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
            kind: MigrationKind::Up,
        },
        Migration {
            version: 11,
            description: "per-run file ledger for sync history",
            // One row per file a run touched; `action` is 'new' | 'updated' | 'unchanged'.
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
            kind: MigrationKind::Up,
        },
        Migration {
            version: 12,
            description: "record which subjects each sync run targeted",
            // JSON array of course codes, written at run start so failed runs know theirs.
            sql: r#"ALTER TABLE sync_runs ADD COLUMN subject_codes TEXT;"#,
            kind: MigrationKind::Up,
        },
        Migration {
            version: 13,
            description: "run origin + user-defined sync schedules",
            // `origin` is 'manual' | 'scheduled'. `sync_schedules` is dropped by 18.
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
            kind: MigrationKind::Up,
        },
        Migration {
            version: 14,
            description: "derive last-synced from sync_runs",
            // Last-synced is derived from `sync_runs`, the only clock.
            sql: r#"ALTER TABLE subjects DROP COLUMN last_synced_at;"#,
            kind: MigrationKind::Up,
        },
        Migration {
            version: 15,
            description: "track when a file's content last changed",
            // Stamped only when a scrape's byte-compare says 'new' or 'updated'.
            sql: r#"ALTER TABLE files ADD COLUMN content_changed_at TEXT;"#,
            kind: MigrationKind::Up,
        },
        Migration {
            version: 16,
            description: "llm usage ledger",
            // Retired (BYOK API layer); the table is created and never used.
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
            kind: MigrationKind::Up,
        },
        Migration {
            version: 17,
            description: "chat conversations",
            // Retired (BYOK chat); the tables are created and never used.
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
            kind: MigrationKind::Up,
        },
        Migration {
            version: 18,
            description: "automations (schedules generalised) + inbox",
            // Retired (automations, Inbox); also where `sync_schedules` is dropped.
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
            kind: MigrationKind::Up,
        },
        Migration {
            version: 19,
            description: "calendar: class times + due dates",
            // Keyed by Canvas's context id; a sync replaces a subject's rows wholesale,
            // so a deleted Canvas event disappears. See `calendar.rs`.
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
            kind: MigrationKind::Up,
        },
        Migration {
            version: 20,
            description: "automations: per-trigger firing state",
            // Retired (automations).
            sql: r#"
ALTER TABLE automations ADD COLUMN trigger_state TEXT NOT NULL DEFAULT '{}';
                        "#,
            kind: MigrationKind::Up,
        },
        Migration {
            version: 21,
            description: "inbox: the instruction an item was summarised with",
            // Retired (Inbox).
            sql: r#"
ALTER TABLE inbox_items ADD COLUMN instruction TEXT;
                        "#,
            kind: MigrationKind::Up,
        },
        Migration {
            version: 22,
            description: "calendar: events Oculus writes itself",
            // Oculus's own events: `calendar_events` is replaced wholesale on every
            // sync. `subject_id` clears rather than cascades — these are the user's rows.
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
            kind: MigrationKind::Up,
        },
        Migration {
            version: 23,
            description: "lectures: the second Echo360 source",
            // `has_source2` is what Echo360 offers; `video2_path` is what is on disk.
            sql: r#"
ALTER TABLE lectures ADD COLUMN video2_path TEXT;
ALTER TABLE lectures ADD COLUMN has_source2 INTEGER NOT NULL DEFAULT 0;
                        "#,
            kind: MigrationKind::Up,
        },
        Migration {
            version: 24,
            description: "harness: CLI-agent threads and their timeline",
            // `provider_session_id` is what a restart resumes with; `ref_id` is the
            // provider's tool-call id, so a result finds its call's row.
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
            kind: MigrationKind::Up,
        },
        Migration {
            version: 25,
            description: "harness: the subject a thread is scoped to",
            // NULL is the general thread (the whole library).
            sql: r#"
ALTER TABLE harness_threads ADD COLUMN subject_id INTEGER REFERENCES subjects(id) ON DELETE SET NULL;
                        "#,
            kind: MigrationKind::Up,
        },
        Migration {
            version: 26,
            description: "harness: whether the model has named the thread",
            // Claimed before the naming turn runs, so two turns cannot both pay for it.
            sql: r#"
ALTER TABLE harness_threads ADD COLUMN title_generated INTEGER NOT NULL DEFAULT 0;
                        "#,
            kind: MigrationKind::Up,
        },
        Migration {
            version: 27,
            description: "projects: per-subject boards, tasks and subtasks",
            // `subject_id` clears rather than cascades (the user's planning, not the
            // course's); `columns` is JSON; `position` is REAL so a drag writes one row.
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
            kind: MigrationKind::Up,
        },
        Migration {
            version: 28,
            description: "harness: the provider's handle for each question",
            // What a rewind names: Claude's user-message uuid or Codex's turn id,
            // learned once when the turn goes out. NULL for older rows.
            sql: r#"
ALTER TABLE harness_items ADD COLUMN anchor TEXT;
                        "#,
            kind: MigrationKind::Up,
        },
        Migration {
            version: 29,
            description: "lecture chapters: named topic spans, and how they were made",
            // Derived data, so it cascades. No end column: a chapter ends where the next
            // begins. `chapter_error` holds the failure for the player to show.
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
            kind: MigrationKind::Up,
        },
        Migration {
            version: 30,
            description: "harness: the lecture a thread is scoped to",
            // SET NULL, not CASCADE: the conversation is the student's. Set at creation
            // only, since every CLI binds its instructions at session start.
            sql: r#"
ALTER TABLE harness_threads ADD COLUMN lecture_id TEXT REFERENCES lectures(id) ON DELETE SET NULL;
                        "#,
            kind: MigrationKind::Up,
        },
        Migration {
            version: 31,
            description: "lecture recap: slide-level notes and job status",
            // Replaced by the reading copy in 34.
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
            kind: MigrationKind::Up,
        },
        Migration {
            version: 32,
            description: "lectures: when the recording was last watched",
            // Written only by the app's player. NULL means never watched — no backfill,
            // or every watched lecture would top Continue on first launch.
            sql: r#"
ALTER TABLE lectures ADD COLUMN last_watched_at TEXT;
                        "#,
            kind: MigrationKind::Up,
        },
        Migration {
            version: 33,
            description: "projects: custom tags, and the calendar event a project answers to",
            // `tags` is JSON like `columns`. `event_id` is deliberately not a foreign
            // key: a sync deletes and re-inserts `calendar_events`, which would cascade
            // every link away. It is resolved live instead.
            sql: r#"
ALTER TABLE projects ADD COLUMN tags TEXT NOT NULL DEFAULT '[]';
ALTER TABLE projects ADD COLUMN event_id TEXT;
                        "#,
            kind: MigrationKind::Up,
        },
        Migration {
            version: 34,
            description: "lecture reading copy: the recap replaced by lines",
            // Recap → reading copy (derived, so dropped, not migrated). The CASE guards
            // `json_type`, which raises on malformed JSON; a WHERE's AND need not short-circuit.
            sql: r#"
DROP TABLE IF EXISTS lecture_recap;
CREATE TABLE IF NOT EXISTS lecture_reading (
    lecture_id    TEXT    NOT NULL REFERENCES lectures(id) ON DELETE CASCADE,
    idx           INTEGER NOT NULL,
    start_seconds INTEGER NOT NULL,
    para          INTEGER NOT NULL DEFAULT 0,
    text          TEXT    NOT NULL,
    PRIMARY KEY (lecture_id, idx)
);
ALTER TABLE lectures DROP COLUMN recap_status;
ALTER TABLE lectures DROP COLUMN recapped_at;
ALTER TABLE lectures DROP COLUMN recap_error;
ALTER TABLE lectures ADD COLUMN reading_status TEXT;
ALTER TABLE lectures ADD COLUMN reading_written_at TEXT;
ALTER TABLE lectures ADD COLUMN reading_error TEXT;
UPDATE settings
   SET value = json_set(json_remove(value, '$.lectureRecap'),
                        '$.lectureReading', json_extract(value, '$.lectureRecap'))
 WHERE key = 'job_models'
   AND CASE WHEN json_valid(value) THEN json_type(value, '$.lectureRecap') END = 'object';
                        "#,
            kind: MigrationKind::Up,
        },
        Migration {
            version: 35,
            description: "lexical page search: an FTS5 index over pages.markdown",
            // The SQL and its triggers live beside the table: `retrieval::PAGES_FTS_SQL`.
            sql: crate::retrieval::PAGES_FTS_SQL,
            kind: MigrationKind::Up,
        },
        Migration {
            version: 36,
            description: "in-app browser: visit history and cached site icons",
            // One row per URL, not per visit; icons are cached per host.
            sql: r#"
CREATE TABLE IF NOT EXISTS browser_history (
    id         INTEGER PRIMARY KEY,
    url        TEXT    NOT NULL UNIQUE,
    host       TEXT    NOT NULL,
    title      TEXT    NOT NULL DEFAULT '',
    visits     INTEGER NOT NULL DEFAULT 1,
    last_visit TEXT    NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_browser_history_recent
    ON browser_history(last_visit DESC);

CREATE TABLE IF NOT EXISTS browser_favicons (
    host       TEXT PRIMARY KEY,
    icon       TEXT NOT NULL,
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);
                        "#,
            kind: MigrationKind::Up,
        },
        Migration {
            version: 37,
            description: "project tasks: a task can belong to no project at all",
            // `project_id` becomes nullable (NULL = unfiled) via a table rebuild. See
            // `projects::UNFILED_TASKS_SQL` and `docs/projects.md` for the self-referencing FK trap.
            sql: crate::projects::UNFILED_TASKS_SQL,
            kind: MigrationKind::Up,
        },
        Migration {
            version: 38,
            description: "lexical page search: reindex only changed markdown",
            sql: crate::retrieval::PAGES_FTS_CHANGED_SQL,
            kind: MigrationKind::Up,
        },
        Migration {
            version: 39,
            description: "document versions: checkpoints and automatic snapshots of a note's text",
            // Whole text per version; `number` only on checkpoints. Rows are deleted by
            // hand with their file (`deleteFileRow`), since the cascade needs `foreign_keys`.
            sql: r#"
CREATE TABLE IF NOT EXISTS document_versions (
    id         INTEGER PRIMARY KEY AUTOINCREMENT,
    file_id    INTEGER NOT NULL REFERENCES files(id) ON DELETE CASCADE,
    number     INTEGER,
    label      TEXT,
    kind       TEXT    NOT NULL CHECK (kind IN ('checkpoint', 'auto', 'external', 'restore')),
    text       TEXT    NOT NULL,
    hash       TEXT    NOT NULL,
    created_at TEXT    NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_document_versions_file
    ON document_versions(file_id, created_at DESC);

CREATE UNIQUE INDEX IF NOT EXISTS idx_document_versions_number
    ON document_versions(file_id, number) WHERE number IS NOT NULL;
                        "#,
            kind: MigrationKind::Up,
        },
        Migration {
            version: 40,
            description: "usage: open and active seconds per local hour",
            // Written only by the ticker in `usage.rs`; open vs active is defined there.
            sql: r#"
CREATE TABLE IF NOT EXISTS usage_hours (
    hour           TEXT    PRIMARY KEY,           -- local time, 'YYYY-MM-DD HH'
    open_seconds   INTEGER NOT NULL DEFAULT 0,
    active_seconds INTEGER NOT NULL DEFAULT 0
);
                        "#,
            kind: MigrationKind::Up,
        },
        Migration {
            version: 41,
            description: "usage: active seconds per local hour, page kind and subject",
            // Written only by the ticker in `usage.rs`, in the transaction that adds
            // the same active seconds to `usage_hours`.
            sql: r#"
CREATE TABLE IF NOT EXISTS usage_context_hours (
    hour           TEXT    NOT NULL,              -- local time, 'YYYY-MM-DD HH'
    kind           TEXT    NOT NULL,              -- a UsageKind from app/src/lib/usageContext.ts
    subject_id     INTEGER NOT NULL DEFAULT 0,    -- 0 outside a subject
    active_seconds INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY (hour, kind, subject_id)
);
                        "#,
            kind: MigrationKind::Up,
        },
    ]
}

#[cfg(test)]
mod tests {
    #[test]
    fn versions_strictly_increase() {
        let versions: Vec<i64> = super::all().iter().map(|migration| migration.version).collect();
        assert!(versions.windows(2).all(|pair| pair[0] < pair[1]), "{versions:?}");
    }

    #[test]
    fn applied_task_rebuild_sql_keeps_its_checksum() {
        let migration = super::all().into_iter().find(|migration| migration.version == 37).unwrap();
        let migration = sqlx::migrate::Migration::new(
            migration.version, migration.description.into(),
            sqlx::migrate::MigrationType::Simple, migration.sql.into(), false,
        );
        let checksum: String = migration.checksum.iter().map(|byte| format!("{byte:02x}")).collect();
        assert_eq!(checksum, "5b74e95b4722a907975ca2d3987acc2450993baeb45734c2b327c92821c54bf18243b1ff91d7225967de9ec955470809",
            "SQL comments and whitespace are part of an applied migration's identity");
    }
}
