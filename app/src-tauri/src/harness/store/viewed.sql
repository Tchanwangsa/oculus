-- When the student last had each thread on screen: a run that ends after it
-- leaves the thread unread. Threads from before count as seen at the upgrade.
ALTER TABLE harness_threads ADD COLUMN viewed_at TEXT;
UPDATE harness_threads SET viewed_at = datetime('now');
