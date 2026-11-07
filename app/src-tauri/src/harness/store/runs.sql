-- App-owned identities for agent turns (docs/harness.md, "Runs"). Rows from
-- before this migration keep NULL ownership: legacy, never back-filled.
CREATE TABLE harness_bindings (
    id             INTEGER PRIMARY KEY,
    thread_id      INTEGER NOT NULL REFERENCES harness_threads(id) ON DELETE CASCADE,
    provider       TEXT    NOT NULL,
    native_session TEXT,
    created_at     TEXT    NOT NULL DEFAULT (datetime('now'))
);
CREATE INDEX idx_harness_bindings_thread ON harness_bindings(thread_id);

CREATE TABLE harness_runs (
    id                 INTEGER PRIMARY KEY,
    thread_id          INTEGER NOT NULL REFERENCES harness_threads(id) ON DELETE CASCADE,
    ordinal            INTEGER NOT NULL,
    origin             TEXT    NOT NULL,
    initiating_item_id INTEGER REFERENCES harness_items(id) ON DELETE SET NULL,
    binding_id         INTEGER NOT NULL REFERENCES harness_bindings(id),
    model              TEXT,
    active_attempt_id  INTEGER,
    execution_status   TEXT    NOT NULL,
    delivery_state     TEXT    NOT NULL DEFAULT 'unknown',
    answer_item_id     INTEGER REFERENCES harness_items(id) ON DELETE SET NULL,
    answer_reason      TEXT,
    requested_at       TEXT    NOT NULL DEFAULT (datetime('now')),
    started_at         TEXT,
    ended_at           TEXT,
    UNIQUE (thread_id, ordinal)
);

CREATE TABLE harness_attempts (
    id         INTEGER PRIMARY KEY,
    run_id     INTEGER NOT NULL REFERENCES harness_runs(id) ON DELETE CASCADE,
    ordinal    INTEGER NOT NULL,
    reason     TEXT    NOT NULL,
    status     TEXT    NOT NULL,
    started_at TEXT,
    ended_at   TEXT,
    UNIQUE (run_id, ordinal)
);

CREATE TABLE harness_command_receipts (
    id         TEXT    PRIMARY KEY,
    thread_id  INTEGER NOT NULL REFERENCES harness_threads(id) ON DELETE CASCADE,
    command    TEXT    NOT NULL,
    run_id     INTEGER NOT NULL REFERENCES harness_runs(id) ON DELETE CASCADE,
    created_at TEXT    NOT NULL DEFAULT (datetime('now'))
);

ALTER TABLE harness_items ADD COLUMN run_id INTEGER REFERENCES harness_runs(id) ON DELETE SET NULL;
ALTER TABLE harness_items ADD COLUMN attempt_id INTEGER REFERENCES harness_attempts(id) ON DELETE SET NULL;
ALTER TABLE harness_items ADD COLUMN entity_ref TEXT;
ALTER TABLE harness_items ADD COLUMN source_ref TEXT;
ALTER TABLE harness_items ADD COLUMN native_phase TEXT;
ALTER TABLE harness_items ADD COLUMN lifecycle TEXT;
ALTER TABLE harness_items ADD COLUMN display_role TEXT;
CREATE INDEX idx_harness_items_run ON harness_items(run_id);
CREATE UNIQUE INDEX idx_harness_items_source
    ON harness_items(attempt_id, kind, source_ref)
    WHERE attempt_id IS NOT NULL AND source_ref IS NOT NULL;

CREATE TABLE harness_delivery_gaps (
    id          INTEGER PRIMARY KEY,
    thread_id   INTEGER NOT NULL REFERENCES harness_threads(id) ON DELETE CASCADE,
    run_id      INTEGER REFERENCES harness_runs(id) ON DELETE CASCADE,
    attempt_id  INTEGER REFERENCES harness_attempts(id) ON DELETE CASCADE,
    source_ref  TEXT,
    cause       TEXT    NOT NULL,
    state       TEXT    NOT NULL DEFAULT 'unresolved',
    observed_at TEXT    NOT NULL DEFAULT (datetime('now'))
);
CREATE INDEX idx_harness_delivery_gaps_run ON harness_delivery_gaps(run_id);

-- Answer text streamed but not yet committed as a row: what a restart keeps.
CREATE TABLE harness_stream_drafts (
    run_id INTEGER PRIMARY KEY REFERENCES harness_runs(id) ON DELETE CASCADE,
    text   TEXT    NOT NULL
);
