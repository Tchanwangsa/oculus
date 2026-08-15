import Database from "@tauri-apps/plugin-sql";

// ── Types ────────────────────────────────────────────────────────────────────

export interface Subject {
  id: number;
  code: string;
  name: string;
  term_name: string | null;
  is_current: boolean;
  workflow_state: string;
  selected: boolean;
  last_synced_at: string | null;
  created_at: string;
}

export interface SyncRun {
  id: number;
  started_at: string;
  finished_at: string | null;
  status: "running" | "completed" | "failed";
  subjects_synced: number;
  pages_scraped: number;
  error: string | null;
}

export interface SyncLogEntry {
  id: number;
  run_id: number | null;
  subject_id: number | null;
  timestamp: string;
  level: "info" | "warning" | "error";
  message: string;
}

export interface DbFile {
  id: number;
  subject_id: number;
  filename: string;
  relative_path: string;
  file_type: string;
  size_bytes: number | null;
  category: string | null;
  canvas_id: number | null;
  source_url: string | null;
  modified_at: string | null;
  scraped_at: string;
  parse_status: string | null;
  parsed_at: string | null;
  /** 'done' once every page has a stored embedding. See lib/retrieval.ts. */
  embed_status: string | null;
  embedded_at: string | null;
}

// ── Singleton ────────────────────────────────────────────────────────────────

let _db: Database | null = null;

export async function getDb(): Promise<Database> {
  if (!_db) {
    _db = await Database.load("sqlite:oculus.db");
  }
  return _db;
}

// ── Subjects ─────────────────────────────────────────────────────────────────

export interface CanvasCourseRaw {
  id: number;
  course_code: string;
  name: string;
  workflow_state: string;
  term?: { name: string };
  _oculus_is_current: boolean;
}

export async function upsertSubjects(courses: CanvasCourseRaw[]): Promise<void> {
  const db = await getDb();
  for (const c of courses) {
    await db.execute(
      `INSERT INTO subjects (id, code, name, term_name, is_current, workflow_state, selected)
       VALUES ($1, $2, $3, $4, $5, $6, 1)
       ON CONFLICT(id) DO UPDATE SET
         name           = excluded.name,
         term_name      = excluded.term_name,
         is_current     = excluded.is_current,
         workflow_state = excluded.workflow_state`,
      [
        c.id,
        c.course_code,
        c.name,
        c.term?.name ?? null,
        c._oculus_is_current ? 1 : 0,
        c.workflow_state,
      ]
    );
  }
}

export async function getSubjects(): Promise<Subject[]> {
  const db = await getDb();
  const rows = await db.select<Subject[]>(
    `SELECT * FROM subjects ORDER BY is_current DESC, term_name DESC, name ASC`
  );
  return rows.map((r) => ({ ...r, is_current: !!r.is_current, selected: !!r.selected }));
}

export async function setSubjectSelected(id: number, selected: boolean): Promise<void> {
  const db = await getDb();
  await db.execute(`UPDATE subjects SET selected = $1 WHERE id = $2`, [selected ? 1 : 0, id]);
}

export async function markSubjectSynced(id: number): Promise<void> {
  const db = await getDb();
  await db.execute(
    `UPDATE subjects SET last_synced_at = datetime('now') WHERE id = $1`,
    [id]
  );
}

// ── Sync runs ────────────────────────────────────────────────────────────────

export async function startSyncRun(): Promise<number> {
  const db = await getDb();
  await db.execute(`INSERT INTO sync_runs (status) VALUES ('running')`);
  const rows = await db.select<{ id: number }[]>(`SELECT last_insert_rowid() AS id`);
  return rows[0].id;
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

/**
 * Fail any sync run left `running` by a previous process.
 *
 * A run is only ever advanced by live events from the scraper, so one that
 * outlives its process can never finish — it just sits at "running" forever and
 * the UI has no way to tell that apart from a slow sync. Call once at startup.
 */
export async function reconcileStaleSyncRuns(): Promise<number> {
  const db = await getDb();
  const stale = await db.select<{ id: number }[]>(
    `SELECT id FROM sync_runs WHERE status = 'running' AND finished_at IS NULL`,
  );
  if (stale.length === 0) return 0;
  await db.execute(
    `UPDATE sync_runs
     SET status = 'failed', finished_at = datetime('now'),
         error = COALESCE(error, 'Interrupted — app closed or sync stalled')
     WHERE status = 'running' AND finished_at IS NULL`,
  );
  return stale.length;
}

export async function getRecentSyncRuns(limit = 10): Promise<SyncRun[]> {
  const db = await getDb();
  return db.select<SyncRun[]>(
    `SELECT * FROM sync_runs ORDER BY started_at DESC LIMIT $1`,
    [limit]
  );
}

export async function getLastCompletedSyncRun(): Promise<SyncRun | null> {
  const db = await getDb();
  const rows = await db.select<SyncRun[]>(
    `SELECT * FROM sync_runs WHERE status = 'completed' AND finished_at IS NOT NULL ORDER BY finished_at DESC LIMIT 1`
  );
  return rows[0] ?? null;
}

// ── Sync log ─────────────────────────────────────────────────────────────────

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

export async function getRecentLogs(limit = 50): Promise<SyncLogEntry[]> {
  const db = await getDb();
  return db.select<SyncLogEntry[]>(
    `SELECT * FROM sync_log ORDER BY timestamp DESC LIMIT $1`,
    [limit]
  );
}

// ── Settings ─────────────────────────────────────────────────────────────────

export async function getSetting(key: string): Promise<string | null> {
  const db = await getDb();
  const rows = await db.select<{ value: string }[]>(
    `SELECT value FROM settings WHERE key = $1`,
    [key]
  );
  return rows[0]?.value ?? null;
}

export async function setSetting(key: string, value: string): Promise<void> {
  const db = await getDb();
  await db.execute(
    `INSERT INTO settings (key, value) VALUES ($1, $2)
     ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = datetime('now')`,
    [key, value]
  );
}

// ── Files ────────────────────────────────────────────────────────────────────

export async function upsertFile(
  subjectId: number,
  filename: string,
  relativePath: string,
  fileType: string,
  sizeBytes?: number,
  category?: string,
  canvasId?: number,
  sourceUrl?: string
): Promise<void> {
  const db = await getDb();
  await db.execute(
    `INSERT INTO files (subject_id, filename, relative_path, file_type, size_bytes, category, canvas_id, source_url)
     VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
     ON CONFLICT(subject_id, relative_path) DO UPDATE SET
       filename   = excluded.filename,
       file_type  = excluded.file_type,
       size_bytes = excluded.size_bytes,
       category   = excluded.category,
       canvas_id  = excluded.canvas_id,
       source_url = excluded.source_url,
       scraped_at = datetime('now')`,
    [subjectId, filename, relativePath, fileType, sizeBytes ?? null,
     category ?? null, canvasId ?? null, sourceUrl ?? null]
  );
}

export async function getFilesForSubject(subjectId: number): Promise<DbFile[]> {
  const db = await getDb();
  return db.select<DbFile[]>(
    `SELECT * FROM files WHERE subject_id = $1 ORDER BY relative_path ASC`,
    [subjectId]
  );
}

/** Update a PDF's parse status. status: 'fast' | 'quality' | 'error' | 'queued' | 'running' */
export async function setParseStatus(
  subjectId: number,
  relativePath: string,
  status: string,
): Promise<void> {
  const db = await getDb();
  const setParsedAt = status === "quality" || status === "fast";
  await db.execute(
    `UPDATE files SET parse_status = $1${setParsedAt ? ", parsed_at = datetime('now')" : ""}
     WHERE subject_id = $2 AND relative_path = $3`,
    [status, subjectId, relativePath],
  );
}

/** Bulk-set parse status by relative_path (used by disk reconciliation). */
export async function setParseStatusByPath(
  entries: Array<[string, string]>,
): Promise<void> {
  if (entries.length === 0) return;
  const db = await getDb();
  for (const [relativePath, status] of entries) {
    await db.execute(
      `UPDATE files SET parse_status = $1, parsed_at = datetime('now')
       WHERE relative_path = $2 AND (parse_status IS NULL OR parse_status != $1)`,
      [status, relativePath],
    );
  }
}

export async function clearAllFiles(): Promise<number> {
  const db = await getDb();
  await db.execute("DELETE FROM files");
  await db.execute("UPDATE subjects SET last_synced_at = NULL");
  await db.execute("VACUUM");
  const rows = await db.select<{ cnt: number }[]>("SELECT COUNT(*) AS cnt FROM files");
  return rows[0]?.cnt ?? 0;
}

// ── Lectures ──────────────────────────────────────────────────────────────────

export interface Lecture {
  id: string;
  lesson_id: string;
  subject_id: number;
  title: string;
  date: string;
  duration_seconds: number;
  video_path: string | null;
  transcript_path: string | null;
  progress_seconds: number;
  completed: number;
  synced_at: string;
}

export interface LectureData {
  id: string;
  lesson_id: string;
  title: string;
  date: string;
  duration_seconds: number;
}

export async function upsertLectures(subjectId: number, lectures: LectureData[]): Promise<void> {
  const db = await getDb();
  for (const l of lectures) {
    await db.execute(
      `INSERT INTO lectures (id, lesson_id, subject_id, title, date, duration_seconds, synced_at)
       VALUES ($1, $2, $3, $4, $5, $6, datetime('now'))
       ON CONFLICT(id) DO UPDATE SET
         title            = excluded.title,
         date             = excluded.date,
         duration_seconds = excluded.duration_seconds,
         synced_at        = datetime('now')`,
      [l.id, l.lesson_id, subjectId, l.title, l.date, l.duration_seconds]
    );
  }
}

export async function getLectures(subjectId: number): Promise<Lecture[]> {
  const db = await getDb();
  return db.select<Lecture[]>(
    `SELECT * FROM lectures WHERE subject_id = $1 ORDER BY date ASC`,
    [subjectId]
  );
}

export async function updateLectureVideoPath(id: string, path: string): Promise<void> {
  const db = await getDb();
  await db.execute(`UPDATE lectures SET video_path = $1 WHERE id = $2`, [path, id]);
}

export async function updateLectureTranscriptPath(id: string, path: string): Promise<void> {
  const db = await getDb();
  await db.execute(`UPDATE lectures SET transcript_path = $1 WHERE id = $2`, [path, id]);
}

export async function updateLectureProgress(id: string, seconds: number): Promise<void> {
  const db = await getDb();
  await db.execute(`UPDATE lectures SET progress_seconds = $1 WHERE id = $2`, [seconds, id]);
}

export async function markLectureComplete(id: string): Promise<void> {
  const db = await getDb();
  await db.execute(
    `UPDATE lectures SET completed = 1, progress_seconds = duration_seconds WHERE id = $1`,
    [id]
  );
}

export async function clearLectureTranscripts(): Promise<void> {
  const db = await getDb();
  await db.execute(`UPDATE lectures SET transcript_path = NULL`);
}
