import { getDb } from "./connection";
import type {
  SyncFileAction,
  SyncRunFile,
  SyncRunSummary,
} from "./types";

export async function startSyncRun(subjectCodes: string[]): Promise<number> {
  const db = await getDb();
  // The id must come from execute()'s result: `SELECT last_insert_rowid()`
  // may run on another pooled connection and return someone else's id.
  const res = await db.execute(
    `INSERT INTO sync_runs (status, subject_codes, origin) VALUES ('running', $1, $2)`,
    [JSON.stringify(subjectCodes), "manual"],
  );
  if (res.lastInsertId == null) throw new Error("sync run insert returned no id");
  return res.lastInsertId;
}

export async function finishSyncRun(
  id: number,
  status: "completed" | "failed",
  subjectsSynced: number,
  pagesScraped: number,
  error?: string
): Promise<void> {
  const db = await getDb();
  await db.execute(
    `UPDATE sync_runs
     SET finished_at = datetime('now'), status = $1,
         subjects_synced = $2, pages_scraped = $3, error = $4
     WHERE id = $5`,
    [status, subjectsSynced, pagesScraped, error ?? null, id]
  );
}

/** Stamped on runs reconciled at startup. Their `finished_at` is the next
 *  launch, not when the sync died, so the UI must not show a duration. */
export const INTERRUPTED_SYNC_ERROR = "Interrupted — app closed or sync stalled";

/** Fail any run left `running` by a previous process — only live scraper
 *  events advance a run, so it could never finish. Call once at startup. */
export async function reconcileStaleSyncRuns(): Promise<number> {
  const db = await getDb();
  const stale = await db.select<{ id: number }[]>(
    `SELECT id FROM sync_runs WHERE status = 'running' AND finished_at IS NULL`,
  );
  if (stale.length === 0) return 0;
  await db.execute(
    `UPDATE sync_runs
     SET status = 'failed', finished_at = datetime('now'),
         error = COALESCE(error, '${INTERRUPTED_SYNC_ERROR}')
     WHERE status = 'running' AND finished_at IS NULL`,
  );
  return stale.length;
}

export async function addSyncRunFile(
  runId: number,
  subjectId: number,
  relativePath: string,
  action: SyncFileAction,
  sizeBytes?: number,
): Promise<void> {
  const db = await getDb();
  await db.execute(
    `INSERT INTO sync_run_files (run_id, subject_id, relative_path, action, size_bytes)
     VALUES ($1, $2, $3, $4, $5)`,
    [runId, subjectId, relativePath, action, sizeBytes ?? null],
  );
}

/** Recent runs, newest first, each with its file ledger rolled up. */
export async function getSyncRunSummaries(limit = 50): Promise<SyncRunSummary[]> {
  const db = await getDb();
  return db.select<SyncRunSummary[]>(
    `SELECT r.*,
            COALESCE(SUM(f.action = 'new'), 0)       AS new_count,
            COALESCE(SUM(f.action = 'updated'), 0)   AS updated_count,
            COALESCE(SUM(f.action = 'unchanged'), 0) AS unchanged_count,
            COUNT(f.id)                              AS file_count
     FROM sync_runs r
     LEFT JOIN sync_run_files f ON f.run_id = r.id
     GROUP BY r.id
     ORDER BY r.started_at DESC, r.id DESC
     LIMIT $1`,
    [limit],
  );
}

/** Every file one run touched — changed files first, then unchanged. */
export async function getSyncRunFiles(runId: number): Promise<SyncRunFile[]> {
  const db = await getDb();
  return db.select<SyncRunFile[]>(
    `SELECT f.*, s.code AS subject_code
     FROM sync_run_files f
     LEFT JOIN subjects s ON s.id = f.subject_id
     WHERE f.run_id = $1
     ORDER BY CASE f.action WHEN 'new' THEN 0 WHEN 'updated' THEN 1 ELSE 2 END,
              f.relative_path ASC`,
    [runId],
  );
}

export async function addLog(
  message: string,
  level: "info" | "warning" | "error" = "info",
  runId?: number,
  subjectId?: number
): Promise<void> {
  const db = await getDb();
  await db.execute(
    `INSERT INTO sync_log (run_id, subject_id, level, message) VALUES ($1, $2, $3, $4)`,
    [runId ?? null, subjectId ?? null, level, message]
  );
}
