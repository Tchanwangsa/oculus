-- Changing provider mid-chat (docs/harness.md, "Changing provider mid-chat").
-- A run records what it was sent with and what its CLI reported; the
-- thread's provider, model and effort are what the next send will use.
ALTER TABLE harness_runs ADD COLUMN effort TEXT;
ALTER TABLE harness_runs ADD COLUMN resolved_model TEXT;
ALTER TABLE harness_threads ADD COLUMN effort TEXT;

-- One current binding per chat and provider: the session a send resumes.
-- `covered_through` is the last run ordinal that session has seen, by
-- running it or by receiving it in a handoff; the context columns are its
-- last reported occupancy, which a handoff's budget reads.
ALTER TABLE harness_bindings ADD COLUMN current INTEGER NOT NULL DEFAULT 0;
ALTER TABLE harness_bindings ADD COLUMN covered_through INTEGER NOT NULL DEFAULT 0;
ALTER TABLE harness_bindings ADD COLUMN context_tokens INTEGER;
ALTER TABLE harness_bindings ADD COLUMN context_window INTEGER;

-- A chat from before bindings resumes from its thread's session.
INSERT INTO harness_bindings (thread_id, provider, native_session)
SELECT t.id, t.provider, t.provider_session_id FROM harness_threads t
 WHERE t.provider_session_id IS NOT NULL
   AND NOT EXISTS (SELECT 1 FROM harness_bindings b
                    WHERE b.thread_id = t.id AND b.provider = t.provider
                      AND b.native_session = t.provider_session_id);

-- Every chat so far has one provider, whose session has seen all of it.
UPDATE harness_bindings
   SET current = 1,
       covered_through = (SELECT COALESCE(MAX(r.ordinal), 0) FROM harness_runs r
                           WHERE r.thread_id = harness_bindings.thread_id AND r.ordinal > 0)
 WHERE id IN (SELECT MAX(b.id) FROM harness_bindings b
                JOIN harness_threads t ON t.id = b.thread_id
               WHERE b.provider = t.provider AND b.native_session IS t.provider_session_id
               GROUP BY b.thread_id);
CREATE UNIQUE INDEX idx_harness_bindings_current
    ON harness_bindings(thread_id, provider) WHERE current = 1;

-- The earlier conversation a run's first prompt carries to a session that
-- has not seen it. `items` is the JSON list of timeline rows sent, `omitted`
-- how many were left out to fit, `bytes` the block's cost. `pending` until
-- the provider has the prompt, then `inline`; `failed` was never delivered.
CREATE TABLE harness_handoffs (
    id              INTEGER PRIMARY KEY,
    thread_id       INTEGER NOT NULL REFERENCES harness_threads(id) ON DELETE CASCADE,
    run_id          INTEGER NOT NULL UNIQUE REFERENCES harness_runs(id) ON DELETE CASCADE,
    from_binding_id INTEGER REFERENCES harness_bindings(id),
    to_binding_id   INTEGER NOT NULL REFERENCES harness_bindings(id),
    kind            TEXT    NOT NULL,
    covered_from    INTEGER NOT NULL,
    covered_to      INTEGER NOT NULL,
    items           TEXT    NOT NULL,
    omitted         INTEGER NOT NULL DEFAULT 0,
    bytes           INTEGER NOT NULL DEFAULT 0,
    status          TEXT    NOT NULL,
    error           TEXT,
    created_at      TEXT    NOT NULL DEFAULT (datetime('now'))
);
CREATE INDEX idx_harness_handoffs_thread ON harness_handoffs(thread_id);
