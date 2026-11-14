-- One command path for what the student does to a chat, and a queued
-- message as a run (docs/harness.md, "One command path", "One turn at a time").
-- A receipt outlives its thread: deleting a chat is a command too.
CREATE TABLE harness_receipts (
    id         TEXT    PRIMARY KEY,
    thread_id  INTEGER NOT NULL,
    command    TEXT    NOT NULL,
    status     TEXT    NOT NULL,
    result     TEXT,
    created_at TEXT    NOT NULL DEFAULT (datetime('now'))
);
CREATE INDEX idx_harness_receipts_thread ON harness_receipts(thread_id);

-- Raised by every committed change to the thread's rows or runs.
ALTER TABLE harness_threads ADD COLUMN revision INTEGER NOT NULL DEFAULT 0;

-- A run that waits its turn: where in the queue, whether the queue is held,
-- and what will be sent. `stop_requested_at` marks a run the student stopped.
ALTER TABLE harness_runs ADD COLUMN queue_position INTEGER;
ALTER TABLE harness_runs ADD COLUMN queue_held INTEGER NOT NULL DEFAULT 0;
ALTER TABLE harness_runs ADD COLUMN queued_text TEXT;
ALTER TABLE harness_runs ADD COLUMN queued_options TEXT;
ALTER TABLE harness_runs ADD COLUMN stop_requested_at TEXT;
CREATE INDEX idx_harness_runs_thread_status ON harness_runs(thread_id, execution_status);
