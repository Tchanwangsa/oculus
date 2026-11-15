-- What an agent asks the student mid-turn: an approval now, a question
-- later (docs/harness.md, "Requests"). A timeline row of kind `request`
-- shows one by id; its state lives here. `responder` is `live` while the
-- session `generation` that raised it can still be answered, `message` for a
-- question answered as a new message, `not_resumable` with a `reason`.
-- `native_ref` is the provider's own callback id as JSON, kept as sent.
CREATE TABLE harness_requests (
    id          INTEGER PRIMARY KEY,
    thread_id   INTEGER NOT NULL REFERENCES harness_threads(id) ON DELETE CASCADE,
    run_id      INTEGER REFERENCES harness_runs(id) ON DELETE CASCADE,
    attempt_id  INTEGER REFERENCES harness_attempts(id) ON DELETE CASCADE,
    binding_id  INTEGER REFERENCES harness_bindings(id) ON DELETE SET NULL,
    item_id     INTEGER REFERENCES harness_items(id) ON DELETE CASCADE,
    provider    TEXT    NOT NULL,
    kind        TEXT    NOT NULL,
    action      TEXT    NOT NULL,
    payload     TEXT    NOT NULL,
    options     TEXT    NOT NULL,
    status      TEXT    NOT NULL DEFAULT 'pending',
    responder   TEXT    NOT NULL,
    generation  INTEGER,
    reason      TEXT,
    native_ref  TEXT    NOT NULL,
    response    TEXT,
    delivery    TEXT    NOT NULL DEFAULT 'none',
    created_at  TEXT    NOT NULL DEFAULT (datetime('now')),
    resolved_at TEXT
);
CREATE INDEX idx_harness_requests_thread_status ON harness_requests(thread_id, status);

-- How the chat's agent asks: `ask` shows each request, `quiet` (shown as
-- "Don't ask") declines without one, `auto` hands it to the CLI's reviewer.
ALTER TABLE harness_threads ADD COLUMN permission_mode TEXT NOT NULL DEFAULT 'ask';
