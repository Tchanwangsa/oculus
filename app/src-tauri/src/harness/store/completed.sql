ALTER TABLE harness_threads ADD COLUMN completed_at TEXT;
CREATE INDEX idx_harness_threads_completed_updated ON harness_threads(completed_at, updated_at);
