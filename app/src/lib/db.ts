import Database from "@tauri-apps/plugin-sql";

// Import cycle: `harness.ts` imports `getDb`/`getSetting` from here. Safe only
// while `isProvider` is called inside function bodies — hoisting it to module
// scope reads `PROVIDERS` in its TDZ and throws at import time.
import { isProvider, type Provider } from "@/lib/harness";
import { PDF_BACKED_SQL_LIST, PIPELINE_SQL_LIST } from "@/lib/fileTypes";
import { compareTermsNewestFirst, TERM_RANK_SQL } from "@/lib/terms";

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

/** `scheduled` is never written now, but old `sync_runs` rows carry it. */
export type SyncOrigin = "manual" | "scheduled";

export interface SyncRun {
  id: number;
  started_at: string;
  finished_at: string | null;
  status: "running" | "completed" | "failed";
  subjects_synced: number;
  pages_scraped: number;
  error: string | null;
  /** JSON array of course codes the run targeted; NULL on old runs. */
  subject_codes: string | null;
  origin: SyncOrigin;
}

export type SyncFileAction = "new" | "updated" | "unchanged";

export interface SyncRunFile {
  id: number;
  run_id: number;
  subject_id: number | null;
  relative_path: string;
  action: SyncFileAction;
  size_bytes: number | null;
  timestamp: string;
  /** Joined from subjects; null if the subject row is gone. */
  subject_code: string | null;
}

/** A sync run plus its per-file ledger rolled up. */
export interface SyncRunSummary extends SyncRun {
  new_count: number;
  updated_count: number;
  unchanged_count: number;
  file_count: number;
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
  /** Sticky and model-blind — see `getUnembeddedPdfs` in lib/retrieval.ts. */
  embed_status: string | null;
  embedded_at: string | null;
  /** Set once on first insert; NULL rows never show as "new". */
  first_seen_at: string | null;
  last_accessed_at: string | null;
  /** When a scrape last found the bytes new or changed (`scraped_at` bumps
   *  every run). Newer than `last_accessed_at` ⇒ the unseen dot returns. */
  content_changed_at: string | null;
}

// ── Singleton ────────────────────────────────────────────────────────────────

let _db: Promise<Database> | null = null;

export function getDb(): Promise<Database> {
  // Share initialization across the shell, restored panes and StrictMode mounts.
  // A failed load must leave the next call free to retry.
  return _db ??= Database.load("sqlite:oculus.db").catch((error) => {
    _db = null;
    throw error;
  });
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
       VALUES ($1, $2, $3, $4, $5, $6, $7)
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
        // New subjects start selected only if current; ON CONFLICT keeps the user's choice.
        c._oculus_is_current ? 1 : 0,
      ]
    );
  }
}

export async function getSubjects(): Promise<Subject[]> {
  const db = await getDb();
  // `last_synced_at` is derived: the latest completed run that targeted the subject.
  const rows = await db.select<Subject[]>(
    `WITH last_sync AS (
       SELECT j.value AS code, MAX(r.finished_at) AS finished_at
       FROM sync_runs r, json_each(r.subject_codes) j
       WHERE r.status = 'completed'
       GROUP BY j.value
     )
     SELECT s.*, ls.finished_at AS last_synced_at
     FROM subjects s LEFT JOIN last_sync ls ON ls.code = s.code
     ORDER BY CAST(substr(s.term_name, 1, 4) AS INTEGER) DESC,
              ${TERM_RANK_SQL("s.term_name")} DESC,
              s.name ASC`
  );

  // `is_current` is recomputed from `terms.ts`, not read from the column: Rust
  // stamps it with `.max()` over term *names*, where "2026 Summer Term" beats
  // "2026 Semester 2" and would mark the real semester as past.
  const latest = rows
    .filter((r) => r.workflow_state === "available")
    .reduce<string | null>(
      (best, r) => (compareTermsNewestFirst(r.term_name, best) < 0 ? r.term_name : best),
      null,
    );

  return rows
    .map((r) => ({
      ...r,
      is_current: r.workflow_state === "available" && r.term_name === latest,
      selected: !!r.selected,
    }))
    // Current term first; sort is stable, so the query's order holds within groups.
    .sort((a, b) => Number(b.is_current) - Number(a.is_current));
}

export async function setSubjectSelected(id: number, selected: boolean): Promise<void> {
  const db = await getDb();
  await db.execute(`UPDATE subjects SET selected = $1 WHERE id = $2`, [selected ? 1 : 0, id]);
}

// ── Sync options ─────────────────────────────────────────────────────────────

/** What a sync fetches. Mirrors `SyncOptions` in `app/src-tauri/src/sync.rs`. */
export interface SyncOptions {
  announcements: boolean;
  /** Assignments and quizzes. */
  assignments: boolean;
  /** Module pages and files. */
  modules: boolean;
  ed: boolean;
  /** Echo360 lecture list only, refreshed by the frontend after the scrape;
   *  never downloads videos. */
  lectures: boolean;
  /** Class times and due dates; also a frontend post-scrape refresh. */
  calendar: boolean;
}

export const DEFAULT_SYNC_OPTIONS: SyncOptions = {
  announcements: true,
  assignments: true,
  modules: true,
  ed: true,
  lectures: true,
  calendar: true,
};

const SYNC_OPTIONS_KEY = "sync-options";

export async function getSyncOptions(): Promise<SyncOptions> {
  const raw = await getSetting(SYNC_OPTIONS_KEY);
  if (!raw) return { ...DEFAULT_SYNC_OPTIONS };
  try {
    // Merge over defaults so options added later default on for old settings.
    return { ...DEFAULT_SYNC_OPTIONS, ...JSON.parse(raw) };
  } catch {
    return { ...DEFAULT_SYNC_OPTIONS };
  }
}

export async function setSyncOptions(options: SyncOptions): Promise<void> {
  await setSetting(SYNC_OPTIONS_KEY, JSON.stringify(options));
}

// ── Per-job models ───────────────────────────────────────────────────────────
//
// Each non-chat model job names its own agent, model and reasoning level. One
// JSON value, read back in Rust by `harness::jobs`, where the jobs run.

/** Mirrors `Job` in `app/src-tauri/src/harness/jobs.rs`. */
export type JobId =
  | "lectureChapters"
  | "lectureReading"
  | "lectureEnd"
  | "threadNaming"
  | "documentSuggestions";

/** `reasoningEffort` is null only for a model that takes no level. */
export interface JobSelection {
  provider: Provider;
  model: string;
  reasoningEffort: string | null;
}

export type JobModels = Record<JobId, JobSelection>;

/** The jobs Settings → Jobs lists, in the order it lists them. */
export const JOBS: { id: JobId; label: string; description: string }[] = [
  {
    id: "lectureChapters",
    label: "Lecture chapters",
    description:
      "Reads a recording's slide frames and transcript and names its topics. One long turn, eight to eleven minutes.",
  },
  {
    id: "lectureReading",
    label: "Lecture reading copy",
    description:
      "Rewrites the transcript as readable text — one sentence per line, pinned to its second, with spoken maths set as maths. One agent turn per ten minutes.",
  },
  {
    id: "lectureEnd",
    label: "Lecture end",
    description:
      "Finds where a lecture's content ends, so Done and Up Next don't wait for the Q&A. One short turn per lecture.",
  },
  {
    id: "threadNaming",
    label: "Chat thread names",
    description:
      "One line naming a conversation from its first exchange, once, after the first reply.",
  },
  {
    id: "documentSuggestions",
    label: "Document suggestions",
    description:
      "Inline ghost-text completions in notes, half a second after you stop typing. One short turn per pause.",
  },
];

/** Mirrors `default_selection` in `app/src-tauri/src/harness/jobs.rs`; either
 *  side may resolve an unconfigured job, so they must agree. */
export const DEFAULT_JOB_MODELS: JobModels = {
  lectureChapters: { provider: "codex", model: "gpt-5.6-luna", reasoningEffort: "xhigh" },
  lectureReading: { provider: "codex", model: "gpt-5.6-luna", reasoningEffort: "medium" },
  lectureEnd: { provider: "claude", model: "claude-haiku-4-5-20251001", reasoningEffort: null },
  threadNaming: { provider: "claude", model: "claude-haiku-4-5-20251001", reasoningEffort: null },
  documentSuggestions: { provider: "claude", model: "claude-sonnet-5-5", reasoningEffort: "medium" },
};

const JOB_MODELS_KEY = "job_models";

/** A missing or malformed job row falls back to its default. */
export async function getJobModels(): Promise<JobModels> {
  const raw = await getSetting(JOB_MODELS_KEY);
  if (!raw) return structuredClone(DEFAULT_JOB_MODELS);
  try {
    const parsed = JSON.parse(raw);
    const out = structuredClone(DEFAULT_JOB_MODELS);
    for (const job of JOBS) {
      const row = parsed?.[job.id];
      if (!row || typeof row.model !== "string" || !row.model.trim()) continue;
      // Checked against `PROVIDERS`, not a hardcoded list that falls behind.
      if (!isProvider(row.provider)) continue;
      out[job.id] = {
        provider: row.provider,
        model: row.model,
        reasoningEffort: typeof row.reasoningEffort === "string" ? row.reasoningEffort : null,
      };
    }
    return out;
  } catch {
    return structuredClone(DEFAULT_JOB_MODELS);
  }
}

export async function setJobModels(models: JobModels): Promise<void> {
  await setSetting(JOB_MODELS_KEY, JSON.stringify(models));
}

// ── Sync runs ────────────────────────────────────────────────────────────────

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
    `INSERT INTO files (subject_id, filename, relative_path, file_type, size_bytes, category, canvas_id, source_url, first_seen_at)
     VALUES ($1, $2, $3, $4, $5, $6, $7, $8, datetime('now'))
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

/** A scrape write changed this file's bytes ('new'/'updated', never
 *  'unchanged') — brings the unseen dot back. */
export async function markFileContentChanged(
  subjectId: number,
  relativePath: string,
): Promise<void> {
  const db = await getDb();
  await db.execute(
    `UPDATE files SET content_changed_at = datetime('now')
     WHERE subject_id = $1 AND relative_path = $2`,
    [subjectId, relativePath],
  );
}

/** Forget a file's parse and embed state after a re-scrape changed its bytes,
 *  so both stages re-run and no stale page vector is ranked. Rust purges the
 *  on-disk artifacts. */
export async function resetFilePipeline(
  subjectId: number,
  relativePath: string,
): Promise<void> {
  const db = await getDb();
  await db.execute(
    `DELETE FROM pages WHERE file_id IN
       (SELECT id FROM files WHERE subject_id = $1 AND relative_path = $2)`,
    [subjectId, relativePath],
  );
  await db.execute(
    `UPDATE files SET parse_status = NULL, parsed_at = NULL,
                      embed_status = NULL, embedded_at = NULL
     WHERE subject_id = $1 AND relative_path = $2`,
    [subjectId, relativePath],
  );
}

/** Drop a file's row with its indexed pages and a note's versions. Those are
 *  deleted explicitly rather than trusting `ON DELETE CASCADE`, which only
 *  fires on a connection with `foreign_keys` on. */
export async function deleteFileRow(id: number): Promise<void> {
  const db = await getDb();
  await db.execute(`DELETE FROM pages WHERE file_id = $1`, [id]);
  await db.execute(`DELETE FROM document_versions WHERE file_id = $1`, [id]);
  await db.execute(`DELETE FROM files WHERE id = $1`, [id]);
}

export async function markFileAccessed(id: number): Promise<void> {
  const db = await getDb();
  await db.execute(`UPDATE files SET last_accessed_at = datetime('now') WHERE id = $1`, [id]);
}

/** Record that a file's bytes changed under the app's hand (editor save, or
 *  the chat agent). `seen` — the student's own save — also stamps
 *  `last_accessed_at`, so the unseen dot lights only for changes they didn't make. */
export async function touchFileRow(
  id: number,
  sizeBytes: number,
  seen: boolean,
): Promise<void> {
  const db = await getDb();
  await db.execute(
    seen
      ? `UPDATE files SET size_bytes = $1, modified_at = datetime('now'),
                          content_changed_at = datetime('now'),
                          last_accessed_at = datetime('now')
         WHERE id = $2`
      : `UPDATE files SET size_bytes = $1, modified_at = datetime('now'),
                          content_changed_at = datetime('now')
         WHERE id = $2`,
    [sizeBytes, id],
  );
}

/** Follow a file that moved on disk, keeping its id so `pages` stay attached. */
export async function renameFileRow(
  id: number,
  filename: string,
  relativePath: string,
): Promise<void> {
  const db = await getDb();
  await db.execute(
    `UPDATE files SET filename = $1, relative_path = $2, modified_at = datetime('now')
     WHERE id = $3`,
    [filename, relativePath, id],
  );
}

/** A file the chat composer's `@` can point the agent at. */
export interface MentionFile {
  id: number;
  subject_id: number;
  subject_code: string;
  filename: string;
  /** Library-root path (`courses/<subject>/…`), as `oculus read` takes it. */
  relative_path: string;
  category: string | null;
}

/**
 * The `@` query's words as AND-ed `LIKE` predicates over `f.filename` (any
 * order), with params bound from `$from` on. Shared by the menu and its
 * "no markdown yet" count so both agree on what matches.
 *
 * Callers must number placeholders in the order the statement's *text*
 * mentions them: SQLite treats `$1` as a name and assigns indices by first
 * appearance, so an out-of-order placeholder binds a neighbour's value.
 */
function mentionMatch(
  query: string,
  from: number,
): { where: string; params: string[]; prefix: string } {
  const words = terms(query);
  return {
    where: words.length
      ? words.map((_, i) => `f.filename LIKE $${from + i} ESCAPE '\\'`).join(" AND ")
      : "1",
    params: words.map((w) => `%${w}%`),
    prefix: `${words[0] ?? ""}%`,
  };
}

/**
 * Candidates for an `@` mention, narrowed to the chat's subject when it has
 * one. Only files the agent can read: `.md`, or a parsed document
 * (`parse_status = 'quality'` is the finished-parse marker). Ranked by a
 * prefix match on the first word, then recency.
 */
export async function searchMentionFiles(
  subjectId: number | null,
  query: string,
  limit = 8,
): Promise<MentionFile[]> {
  const db = await getDb();
  const { where, params, prefix } = mentionMatch(query, 2);
  return db.select<MentionFile[]>(
    `SELECT f.id, f.subject_id, s.code AS subject_code, f.filename,
            f.relative_path, f.category
     FROM files f
     JOIN subjects s ON s.id = f.subject_id
     WHERE (f.file_type = 'md' OR f.parse_status = 'quality')
       AND ($1 IS NULL OR f.subject_id = $1)
       AND (${where})
     ORDER BY (f.filename LIKE $${params.length + 2} ESCAPE '\\') DESC,
              f.last_accessed_at DESC,
              f.filename ASC
     LIMIT $${params.length + 3}`,
    [subjectId, ...params, prefix, limit],
  );
}

/** A file a note's `@` offers, with its subject's code for the menu. */
export type NoteLinkFile = DbFile & { subject_code: string };

/** Files a note's `@` can mention: every file of `subjectId`, or of the whole
 *  library when it is null, parsed or not (a mention opens the file, nothing
 *  reads it), `excludePath` (the note itself) left out. Ranked like
 *  `searchMentionFiles`, so an empty query lists recently opened files. */
export async function searchNoteLinkFiles(
  subjectId: number | null,
  excludePath: string | null,
  query: string,
  limit = 8,
): Promise<NoteLinkFile[]> {
  const db = await getDb();
  const { where, params, prefix } = mentionMatch(query, 3);
  return db.select<NoteLinkFile[]>(
    `SELECT f.*, s.code AS subject_code
     FROM files f
     JOIN subjects s ON s.id = f.subject_id
     WHERE ($1 IS NULL OR f.subject_id = $1)
       AND ($2 IS NULL OR f.relative_path != $2)
       AND (${where})
     ORDER BY (f.filename LIKE $${params.length + 3} ESCAPE '\\') DESC,
              f.last_accessed_at DESC,
              f.filename ASC
     LIMIT $${params.length + 4}`,
    [subjectId, excludePath, ...params, prefix, limit],
  );
}

/** How many PDF-backed files the `@` query would match if they were parsed,
 *  so the menu can say why they're missing rather than look like a typo. */
export async function countUnparsedMentionMatches(
  subjectId: number | null,
  query: string,
): Promise<number> {
  const db = await getDb();
  const { where, params } = mentionMatch(query, 2);
  const rows = await db.select<{ n: number }[]>(
    `SELECT COUNT(*) AS n
     FROM files f
     WHERE lower(f.file_type) IN ${PDF_BACKED_SQL_LIST}
       AND (f.parse_status IS NULL OR f.parse_status != 'quality')
       AND ($1 IS NULL OR f.subject_id = $1)
       AND (${where})`,
    [subjectId, ...params],
  );
  return rows[0]?.n ?? 0;
}

/** One file by its library path (`courses/<subject>/…`), which includes the
 *  subject folder and so needs no subject id. */
export async function getFileByRelativePath(
  relativePath: string,
): Promise<DbFile | null> {
  const db = await getDb();
  const rows = await db.select<DbFile[]>(
    `SELECT * FROM files WHERE relative_path = $1 LIMIT 1`,
    [relativePath],
  );
  return rows[0] ?? null;
}

/** Files whose library path ends in `/<tail>` — a course-relative path or a
 *  bare filename an agent cited. At most two, so a caller can tell a unique
 *  hit from an ambiguous one. */
export async function findFilesByTail(tail: string): Promise<DbFile[]> {
  const db = await getDb();
  return db.select<DbFile[]>(
    `SELECT * FROM files WHERE relative_path LIKE $1 ESCAPE '\\' LIMIT 2`,
    [`%/${likeEscape(tail)}`],
  );
}

export async function getFilesForSubject(subjectId: number): Promise<DbFile[]> {
  const db = await getDb();
  return db.select<DbFile[]>(
    `SELECT * FROM files WHERE subject_id = $1 ORDER BY relative_path ASC`,
    [subjectId]
  );
}

// ── Palette search ───────────────────────────────────────────────────────────

/** What the palette matches against: the filename with slug separators
 *  flattened to spaces, plus the subject code, so "comp30026 workshop" is one
 *  query. */
const FILE_HAYSTACK = `replace(replace(f.filename, '-', ' '), '_', ' ') || ' ' || s.code`;
const LECTURE_HAYSTACK = `l.title || ' ' || s.code`;

/** Words beyond this are ignored (noise from a pasted line). */
const MAX_TERMS = 6;

export function likeEscape(s: string): string {
  return s.replace(/[%_\\]/g, (c) => `\\${c}`);
}

/** The typed words, escaped for LIKE. Empty matches every row. */
function terms(query: string): string[] {
  return query.trim().split(/\s+/).filter(Boolean).slice(0, MAX_TERMS).map(likeEscape);
}

/**
 * `AND`-ed substring predicates (every word, any order) plus a rank: whether
 * some haystack word *starts* with the first term (a leading space is
 * prepended so the first word counts).
 *
 * `scope` adds `column = value` predicates (skipped when the value is
 * undefined), numbered before the rank so placeholders keep text order (see
 * `mentionMatch`).
 */
export function matchSql(
  haystack: string,
  query: string,
  scope: [column: string, value: string | number | undefined][] = [],
): { where: string; rank: string; params: (string | number)[] } {
  const words = terms(query);
  const preds = words.map((_, i) => `${haystack} LIKE $${i + 1} ESCAPE '\\'`);
  const params: (string | number)[] = words.map((w) => `%${w}%`);
  for (const [column, value] of scope) {
    if (value === undefined) continue;
    params.push(value);
    preds.push(`${column} = $${params.length}`);
  }
  params.push(`% ${words[0] ?? ""}%`);
  const where = preds.length ? preds.join(" AND ") : "1";
  const rank = `((' ' || ${haystack}) LIKE $${params.length} ESCAPE '\\')`;
  return { where, rank, params };
}

/** Narrows a palette search (`in:` / `type:` in `app/src/lib/search.ts`). */
export interface SearchScope {
  subjectId?: number;
  /** A `files.category` (`category_from_path` in `app/src-tauri/src/paths.rs`). */
  category?: string;
}

export interface LibraryFileHit extends DbFile {
  subject_code: string;
}

/** Enough of a `Lecture` for the palette to build its route. */
export interface LibraryLectureHit {
  id: string;
  subject_id: number;
  subject_code: string;
  title: string;
  date: string;
}

/** Files matching a palette query, best first. Unlike the `@` menu, every file
 *  — a person can read an unparsed PDF. Ties go to this term, then recency. */
export async function searchLibraryFiles(
  query: string,
  limit = 8,
  scope: SearchScope = {},
): Promise<LibraryFileHit[]> {
  const db = await getDb();
  const { where, rank, params } = matchSql(FILE_HAYSTACK, query, [
    ["f.subject_id", scope.subjectId],
    ["f.category", scope.category],
  ]);
  return db.select<LibraryFileHit[]>(
    `SELECT f.*, s.code AS subject_code
     FROM files f
     JOIN subjects s ON s.id = f.subject_id
     WHERE ${where}
     ORDER BY ${rank} DESC,
              s.is_current DESC,
              f.last_accessed_at DESC,
              f.filename ASC
     LIMIT $${params.length + 1}`,
    [...params, limit],
  );
}

/** Lectures matching a palette query — same ranking, newest capture first. */
export async function searchLibraryLectures(
  query: string,
  limit = 4,
  scope: Pick<SearchScope, "subjectId"> = {},
): Promise<LibraryLectureHit[]> {
  const db = await getDb();
  const { where, rank, params } = matchSql(LECTURE_HAYSTACK, query, [
    ["l.subject_id", scope.subjectId],
  ]);
  return db.select<LibraryLectureHit[]>(
    `SELECT l.id, l.subject_id, s.code AS subject_code, l.title, l.date
     FROM lectures l
     JOIN subjects s ON s.id = l.subject_id
     WHERE ${where}
     ORDER BY ${rank} DESC,
              s.is_current DESC,
              l.date DESC
     LIMIT $${params.length + 1}`,
    [...params, limit],
  );
}

/** One file whose page text matched: the best page and FTS5's snippet. */
export interface PageTextHit {
  file_id: number;
  subject_id: number;
  subject_code: string;
  relative_path: string;
  filename: string;
  category: string | null;
  page_no: number;
  /** The matched line, with each hit fenced by {@link SNIP_OPEN} /
   *  {@link SNIP_CLOSE}. Parsed by `snippetParts`, never rendered raw. */
  snippet: string;
}

/** The fences `snippet()` wraps a hit in — control characters, because the
 *  markdown can contain any printable delimiter. */
export const SNIP_OPEN = "\u0001";
export const SNIP_CLOSE = "\u0002";

/** Shorter prefix terms match most of the library. */
const MIN_TEXT_QUERY = 2;

/** Terms FTS5 can tokenise (a letter or digit): a token-less phrase like
 *  `"--"` is a syntax error, not an empty result. */
function ftsTerms(query: string): string[] {
  return query
    .trim()
    .split(/\s+/)
    .filter((w) => /[\p{L}\p{N}]/u.test(w))
    .slice(0, MAX_TERMS);
}

/** FTS5 MATCH for what was typed: every word required, each a prefix. Terms
 *  are quoted as FTS5 strings so `AND`/`NOT`/`-`/`:` etc. in typed text are
 *  literal, not query syntax. */
function ftsMatch(query: string): string | null {
  const words = ftsTerms(query);
  if (words.length === 0) return null;
  if (words.join("").length < MIN_TEXT_QUERY) return null;
  return words.map((w) => `"${w.replace(/"/g, '""')}"*`).join(" ");
}

/**
 * Files whose page text matches, best first — the lexical half of search, over
 * `pages_fts` (parsed documents only; see docs/retrieval.md).
 *
 * One row per file: SQLite takes the bare columns beside a single `MIN()` from
 * that row, so `page_no` and `snippet` belong to the best-scoring page. The
 * join onto `pages` also drops stale FTS entries (see `retrieval::PAGES_FTS_SQL`).
 */
export async function searchPageText(
  query: string,
  limit = 5,
  scope: SearchScope = {},
): Promise<PageTextHit[]> {
  const match = ftsMatch(query);
  if (!match) return [];
  const db = await getDb();
  // Scope placeholders follow MATCH's `$3`, in text order.
  const params: (string | number)[] = [SNIP_OPEN, SNIP_CLOSE, match];
  let scoped = "";
  for (const [column, value] of [
    ["f.subject_id", scope.subjectId],
    ["f.category", scope.category],
  ] as const) {
    if (value === undefined) continue;
    params.push(value);
    scoped += ` AND ${column} = $${params.length}`;
  }
  try {
    return await db.select<PageTextHit[]>(
      `SELECT p.file_id        AS file_id,
              f.subject_id     AS subject_id,
              s.code           AS subject_code,
              f.relative_path  AS relative_path,
              f.filename       AS filename,
              f.category       AS category,
              p.page_no        AS page_no,
              snippet(pages_fts, 0, $1, $2, '…', 14) AS snippet,
              MIN(bm25(pages_fts)) AS score
         FROM pages_fts
         JOIN pages p    ON p.id = pages_fts.rowid
         JOIN files f    ON f.id = p.file_id
         JOIN subjects s ON s.id = f.subject_id
        WHERE pages_fts MATCH $3${scoped}
        GROUP BY p.file_id
        ORDER BY score ASC, s.is_current DESC
        LIMIT $${params.length + 1}`,
      [...params, limit],
    );
  } catch (e) {
    // A malformed MATCH is the user still typing, not a broken index.
    console.warn("[oculus] page text search", e);
    return [];
  }
}

/** One file's stages as the DB holds them. */
export interface PdfPipelineRow {
  subject_id: number;
  relative_path: string;
  parse_status: string | null;
  embed_status: string | null;
  /** Rewritten by every sync, changed bytes or not — only a fallback for
   *  the download time below. */
  scraped_at: string | null;
  first_seen_at: string | null;
  content_changed_at: string | null;
  parsed_at: string | null;
  embedded_at: string | null;
}

async function pipelineRows(types: string): Promise<PdfPipelineRow[]> {
  const db = await getDb();
  return db.select<PdfPipelineRow[]>(
    `SELECT subject_id, relative_path, parse_status, embed_status,
            scraped_at, first_seen_at, content_changed_at, parsed_at, embedded_at
     FROM files
     WHERE lower(file_type) IN ${types}
     ORDER BY relative_path ASC`,
  );
}

/** Every PDF-backed file's stages — the parse counts in Settings. */
export function getPdfPipelineRows(): Promise<PdfPipelineRow[]> {
  return pipelineRows(PDF_BACKED_SQL_LIST);
}

/** Every file with a File Activity row (PDF-backed, and spreadsheets) —
 *  seeds the Sync page's pipeline table. */
export function getPipelineRows(): Promise<PdfPipelineRow[]> {
  return pipelineRows(PIPELINE_SQL_LIST);
}

/** status: 'queued' | 'running' | 'quality' | 'error'; `'quality'` is the
 *  one terminal success. */
export async function setParseStatus(
  subjectId: number,
  relativePath: string,
  status: string,
): Promise<boolean> {
  const db = await getDb();
  const setParsedAt = status === "quality";
  const result = await db.execute(
    `UPDATE files SET parse_status = $1${setParsedAt ? ", parsed_at = datetime('now')" : ""}
     WHERE subject_id = $2 AND relative_path = $3`,
    [status, subjectId, relativePath],
  );
  return result.rowsAffected > 0;
}

export interface EmbedCoverageRow {
  relative_path: string;
  /** Page rows the parse wrote; 0 means unparsed, so never covered. */
  pages_total: number;
  pages_current: number;
}

/** How much of each PDF is embedded in the given space — page-vector
 *  coverage, not `files.embed_status` (see `getUnembeddedPdfs`). */
export async function getEmbedCoverage(
  model: string | null,
  dim: number | null,
): Promise<EmbedCoverageRow[]> {
  if (!model || dim == null) return [];
  const db = await getDb();
  return db.select<EmbedCoverageRow[]>(
    `SELECT f.relative_path,
            (SELECT COUNT(*) FROM pages p WHERE p.file_id = f.id) AS pages_total,
            (SELECT COUNT(*) FROM pages p
              WHERE p.file_id = f.id AND p.embedding IS NOT NULL
                AND p.embed_model = $1 AND p.embed_dim = $2) AS pages_current
     FROM files f
     WHERE lower(f.file_type) IN ${PDF_BACKED_SQL_LIST}`,
    [model, dim],
  );
}

/** status: 'done' | 'error'. Mostly for the failure: success shows in page
 *  vectors, but a failure leaves no other trace across a restart. Rust also
 *  writes `'done'` when an ingest commits; `'queued'` is the app queue's own
 *  mark (`markEmbedQueued`). */
export async function setEmbedStatus(
  subjectId: number,
  relativePath: string,
  status: string,
): Promise<void> {
  const db = await getDb();
  const setEmbeddedAt = status === "done";
  await db.execute(
    `UPDATE files SET embed_status = $1${setEmbeddedAt ? ", embedded_at = datetime('now')" : ""}
     WHERE subject_id = $2 AND relative_path = $3`,
    [status, subjectId, relativePath],
  );
}

/** SQLite's bound-variable ceiling is far above this; chunking keeps a whole
 *  backlog's ids from building one enormous statement. */
const ID_CHUNK = 500;

async function updateByIds(sql: (placeholders: string) => string, ids: number[]): Promise<void> {
  if (ids.length === 0) return;
  const db = await getDb();
  for (let i = 0; i < ids.length; i += ID_CHUNK) {
    const chunk = ids.slice(i, i + ID_CHUNK);
    await db.execute(sql(chunk.map((_, j) => `$${j + 1}`).join(", ")), chunk);
  }
}

/** Record files as waiting in the app's embed queue, so the queue survives a
 *  restart (`restoreIndexQueue` in stores/indexStore.ts). */
export function markEmbedQueued(fileIds: number[]): Promise<void> {
  return updateByIds(
    (ids) => `UPDATE files SET embed_status = 'queued' WHERE id IN (${ids})`,
    fileIds,
  );
}

/** Take files out of the persisted queue. Only a row still saying `queued`
 *  is touched: a finished embed's `done` / `error` stands. */
export function clearEmbedQueued(fileIds: number[]): Promise<void> {
  return updateByIds(
    (ids) => `UPDATE files SET embed_status = NULL
              WHERE embed_status = 'queued' AND id IN (${ids})`,
    fileIds,
  );
}

export interface QueuedEmbedRow {
  id: number;
  subject_id: number;
  relative_path: string;
  filename: string;
  /** A parsed, PDF-backed file — what `getUnembeddedPdfs` would offer. */
  embeddable: number;
  pages_total: number;
  pages_current: number;
}

/** Files the persisted embed queue holds, with their coverage in the given
 *  space. Ordered by parse time, the order a parse-fed queue filled in. */
export async function getQueuedEmbedRows(
  model: string | null,
  dim: number | null,
): Promise<QueuedEmbedRow[]> {
  const db = await getDb();
  return db.select<QueuedEmbedRow[]>(
    `SELECT f.id, f.subject_id, f.relative_path, f.filename,
            (lower(f.file_type) IN ${PDF_BACKED_SQL_LIST}
              AND f.parse_status = 'quality') AS embeddable,
            (SELECT COUNT(*) FROM pages p WHERE p.file_id = f.id) AS pages_total,
            (SELECT COUNT(*) FROM pages p
              WHERE p.file_id = f.id AND p.embedding IS NOT NULL
                AND p.embed_model = $1 AND p.embed_dim = $2) AS pages_current
     FROM files f
     WHERE f.embed_status = 'queued'
     ORDER BY f.parsed_at IS NULL, f.parsed_at, f.relative_path`,
    [model, dim],
  );
}

/** Bulk-set parse status by relative_path, for disk reconciliation. A
 *  recorded `parsed_at` is kept: the parse happened then, not when the disk
 *  check noticed it. */
export async function setParseStatusByPath(
  entries: Array<[string, string]>,
): Promise<void> {
  if (entries.length === 0) return;
  const db = await getDb();
  for (const [relativePath, status] of entries) {
    await db.execute(
      `UPDATE files SET parse_status = $1, parsed_at = COALESCE(parsed_at, datetime('now'))
       WHERE relative_path = $2 AND (parse_status IS NULL OR parse_status != $1)`,
      [status, relativePath],
    );
  }
}

// ── Lectures ──────────────────────────────────────────────────────────────────

/** A capture's stream: 1 the presenter screen, 2 the room camera. */
export type SourceNum = 1 | 2;

export interface Lecture {
  id: string;
  lesson_id: string;
  subject_id: number;
  title: string;
  date: string;
  duration_seconds: number;
  video_path: string | null;
  /** The camera stream, downloaded separately and often not at all. */
  video2_path: string | null;
  /** 1 when Echo360 publishes a camera stream for this capture. */
  has_source2: number;
  transcript_path: string | null;
  progress_seconds: number;
  /** UTC `datetime('now')`, comparable with `files.last_accessed_at`; NULL
   *  is never watched. Home's Recent row ranks on it. */
  last_watched_at: string | null;
  completed: number;
  synced_at: string;
  /** `null` (never run) | `running` | `ready` | `error`. */
  chapter_status: string | null;
  /** Stamped only by a terminal status. */
  chaptered_at: string | null;
  /** Why the last run failed; cleared on success. */
  chapter_error: string | null;
  /** Same values as `chapter_status`; the two jobs are independent. */
  reading_status: string | null;
  reading_written_at: string | null;
  reading_error: string | null;
  /** Where the lecture's planned content ends (`lecture_end`): the end of the
   *  sign-off line, in seconds. Kept when a later run fails. */
  content_end_seconds: number | null;
  /** The words of the sign-off the end was found on. */
  content_end_quote: string | null;
  /** `null` (never run) | `running` | `ready` | `none` (cut off, no end) | `error`. */
  content_end_status: string | null;
  content_end_error: string | null;
}

export interface LectureData {
  id: string;
  lesson_id: string;
  title: string;
  date: string;
  duration_seconds: number;
  has_second_source: boolean;
}

/** The column a source's file path is stored in. */
const videoPathColumn = (source: SourceNum) =>
  source === 1 ? "video_path" : "video2_path";

/** A lecture's downloaded file for one source, or null. */
export const videoPathFor = (lec: Lecture, source: SourceNum) =>
  source === 1 ? lec.video_path : lec.video2_path;

export async function upsertLectures(subjectId: number, lectures: LectureData[]): Promise<void> {
  const db = await getDb();
  for (const l of lectures) {
    await db.execute(
      `INSERT INTO lectures
         (id, lesson_id, subject_id, title, date, duration_seconds, has_source2, synced_at)
       VALUES ($1, $2, $3, $4, $5, $6, $7, datetime('now'))
       ON CONFLICT(id) DO UPDATE SET
         title            = excluded.title,
         date             = excluded.date,
         duration_seconds = excluded.duration_seconds,
         has_source2      = excluded.has_source2,
         synced_at        = datetime('now')`,
      [
        l.id,
        l.lesson_id,
        subjectId,
        l.title,
        l.date,
        l.duration_seconds,
        l.has_second_source ? 1 : 0,
      ]
    );
  }
}

export async function getLecture(id: string): Promise<Lecture | null> {
  const db = await getDb();
  const rows = await db.select<Lecture[]>(`SELECT * FROM lectures WHERE id = $1`, [id]);
  return rows[0] ?? null;
}

export async function getLectures(subjectId: number): Promise<Lecture[]> {
  const db = await getDb();
  return db.select<Lecture[]>(
    `SELECT * FROM lectures WHERE subject_id = $1 ORDER BY date ASC`,
    [subjectId]
  );
}

export async function updateLectureVideoPath(
  id: string,
  path: string,
  source: SourceNum = 1
): Promise<void> {
  const db = await getDb();
  // The column name is one of two literals, never user input.
  await db.execute(`UPDATE lectures SET ${videoPathColumn(source)} = $1 WHERE id = $2`, [
    path,
    id,
  ]);
}

/** Forget a deleted download; progress, chapters and notes stay with the
 *  lecture. `source: null` clears both streams, like `echo360_delete_video`. */
export async function clearLectureVideoPath(
  id: string,
  source: SourceNum | null = null
): Promise<void> {
  const db = await getDb();
  const columns = source === null ? ([1, 2] as SourceNum[]) : [source];
  for (const s of columns) {
    // The column name is one of two literals, never user input.
    await db.execute(`UPDATE lectures SET ${videoPathColumn(s)} = NULL WHERE id = $1`, [id]);
  }
}

export async function updateLectureTranscriptPath(id: string, path: string): Promise<void> {
  const db = await getDb();
  await db.execute(`UPDATE lectures SET transcript_path = $1 WHERE id = $2`, [path, id]);
}

/** Saving a position is the record of watching, so it stamps `last_watched_at`. */
export async function updateLectureProgress(id: string, seconds: number): Promise<void> {
  const db = await getDb();
  await db.execute(
    `UPDATE lectures SET progress_seconds = $1, last_watched_at = datetime('now')
     WHERE id = $2`,
    [seconds, id],
  );
}

export async function markLectureComplete(id: string): Promise<void> {
  const db = await getDb();
  await db.execute(
    `UPDATE lectures SET completed = 1, progress_seconds = duration_seconds,
                         last_watched_at = datetime('now')
     WHERE id = $1`,
    [id]
  );
}

/** The list's Done toggle. Marking keeps the position, since it may not have
 *  been watched; unmarking a lecture watched to its end (`rewind`) starts it
 *  over, or it would show nothing left and re-mark itself on the next save. */
export async function setLectureDone(id: string, done: boolean, rewind = false): Promise<void> {
  const db = await getDb();
  await db.execute(
    `UPDATE lectures SET completed = $1,
                         progress_seconds = CASE WHEN $2 THEN 0 ELSE progress_seconds END
     WHERE id = $3`,
    [done ? 1 : 0, !done && rewind ? 1 : 0, id],
  );
}

/** The lecture after `lec` in its subject, Done or not: the earliest strictly
 *  later `date`, ties broken by title then id. What Up Next offers. */
export async function getNextLecture(
  lec: Pick<Lecture, "subject_id" | "date">,
): Promise<(Lecture & { subject_code: string }) | null> {
  const db = await getDb();
  const rows = await db.select<(Lecture & { subject_code: string })[]>(
    `SELECT l.*, s.code AS subject_code
       FROM lectures l
       JOIN subjects s ON s.id = l.subject_id
      WHERE l.subject_id = $1 AND l.date > $2
      ORDER BY l.date ASC, l.title ASC, l.id ASC
      LIMIT 1`,
    [lec.subject_id, lec.date],
  );
  return rows[0] ?? null;
}

/** The end job's columns and the transcript it reads. */
export type LectureEndRow = Pick<
  Lecture,
  "transcript_path" | "content_end_seconds" | "content_end_status" | "content_end_error"
>;

/** Read on its own, as `getChapterStatus` is. */
export async function getLectureEnd(lectureId: string): Promise<LectureEndRow | null> {
  const db = await getDb();
  const rows = await db.select<LectureEndRow[]>(
    `SELECT transcript_path, content_end_seconds, content_end_status, content_end_error
       FROM lectures WHERE id = $1`,
    [lectureId],
  );
  return rows[0] ?? null;
}

// ── Lecture chapters ──────────────────────────────────────────────────────────

/** A `lecture_chapters` row (`app/src-tauri/src/chapters.rs`). No end column:
 *  a chapter runs until the next starts (`chapterSpans` in lib/lectures.ts). */
export interface Chapter {
  lecture_id: string;
  idx: number;
  start_seconds: number;
  title: string;
  summary: string;
}

export async function getChapters(lectureId: string): Promise<Chapter[]> {
  const db = await getDb();
  return db.select<Chapter[]>(
    `SELECT * FROM lecture_chapters WHERE lecture_id = $1 ORDER BY idx ASC`,
    [lectureId],
  );
}

/** Read on its own: the player's `lecture` prop is a snapshot not re-read
 *  when a run lands. */
export async function getChapterStatus(
  lectureId: string,
): Promise<{ chapter_status: string | null; chapter_error: string | null } | null> {
  const db = await getDb();
  const rows = await db.select<
    { chapter_status: string | null; chapter_error: string | null }[]
  >(`SELECT chapter_status, chapter_error FROM lectures WHERE id = $1`, [lectureId]);
  return rows[0] ?? null;
}

// ── Lecture reading copy ──────────────────────────────────────────────────────

/** A `lecture_reading` row (`app/src-tauri/src/reading.rs`): one sentence,
 *  pinned to its second, running until the next. `para` (derived in Rust) is 1
 *  where the panel breaks a paragraph. */
export interface ReadingLine {
  lecture_id: string;
  idx: number;
  start_seconds: number;
  para: number;
  text: string;
}

export async function getReading(lectureId: string): Promise<ReadingLine[]> {
  const db = await getDb();
  return db.select<ReadingLine[]>(
    `SELECT * FROM lecture_reading WHERE lecture_id = $1 ORDER BY idx ASC`,
    [lectureId],
  );
}

/** Read on its own, as `getChapterStatus` is. */
export async function getReadingStatus(
  lectureId: string,
): Promise<{ reading_status: string | null; reading_error: string | null } | null> {
  const db = await getDb();
  const rows = await db.select<
    { reading_status: string | null; reading_error: string | null }[]
  >(`SELECT reading_status, reading_error FROM lectures WHERE id = $1`, [lectureId]);
  return rows[0] ?? null;
}

// ── Calendar ──────────────────────────────────────────────────────────────────

/** A `calendar_events` row joined to its subject. Times are ISO8601 UTC. */
export interface DbCalendarEvent {
  id: string;
  subject_id: number;
  subject_code: string;
  /** `class` or `due`. */
  kind: string;
  title: string;
  start_at: string;
  end_at: string | null;
  all_day: number;
  location: string | null;
  url: string | null;
  description: string | null;
}

/** What `calendar_sync_events` returns — mirrors `CalendarEvent` in
 *  `app/src-tauri/src/calendar.rs`. */
export interface CalendarEventData {
  id: string;
  kind: string;
  title: string;
  start_at: string;
  end_at: string | null;
  all_day: boolean;
  location: string | null;
  url: string | null;
  description: string | null;
}

/** Swap a subject's calendar for the set Canvas just returned. Delete-then-
 *  insert so a cancelled class disappears (as `store::replace_calendar_events`). */
export async function replaceCalendarEvents(
  subjectId: number,
  events: CalendarEventData[],
): Promise<void> {
  const db = await getDb();
  await db.execute(`DELETE FROM calendar_events WHERE subject_id = $1`, [subjectId]);
  for (const e of events) {
    await db.execute(
      `INSERT INTO calendar_events
         (id, subject_id, kind, title, start_at, end_at, all_day, location, url,
          description, synced_at)
       VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, datetime('now'))
       ON CONFLICT(id) DO UPDATE SET
         subject_id  = excluded.subject_id,
         kind        = excluded.kind,
         title       = excluded.title,
         start_at    = excluded.start_at,
         end_at      = excluded.end_at,
         all_day     = excluded.all_day,
         location    = excluded.location,
         url         = excluded.url,
         description = excluded.description,
         synced_at   = datetime('now')`,
      [
        e.id, subjectId, e.kind, e.title, e.start_at, e.end_at,
        e.all_day ? 1 : 0, e.location, e.url, e.description,
      ],
    );
  }
}

/** Every stored calendar event, oldest first — unwindowed; it's a few hundred rows. */
export async function getCalendarEvents(): Promise<DbCalendarEvent[]> {
  const db = await getDb();
  return db.select<DbCalendarEvent[]>(
    `SELECT ce.id, ce.subject_id, s.code AS subject_code, ce.kind, ce.title,
            ce.start_at, ce.end_at, ce.all_day, ce.location, ce.url, ce.description
       FROM calendar_events ce
       JOIN subjects s ON s.id = ce.subject_id
      ORDER BY ce.start_at ASC`,
  );
}

/** Lecture recordings across every subject — the calendar's third layer. */
export async function getAllLectures(): Promise<
  (Lecture & { subject_code: string })[]
> {
  const db = await getDb();
  return db.select<(Lecture & { subject_code: string })[]>(
    `SELECT l.*, s.code AS subject_code
       FROM lectures l
       JOIN subjects s ON s.id = l.subject_id
      ORDER BY l.date ASC`,
  );
}

// ── Recency (Home's Recent row) ───────────────────────────────────────────────

/** Lectures in progress, most recently watched first. The `> 5` must match
 *  the "started" threshold in `progressLabel` (lib/lectureEnd.ts). */
export async function getRecentlyWatchedLectures(
  limit = 8,
): Promise<(Lecture & { subject_code: string })[]> {
  const db = await getDb();
  return db.select<(Lecture & { subject_code: string })[]>(
    `SELECT l.*, s.code AS subject_code
       FROM lectures l
       JOIN subjects s ON s.id = l.subject_id
      WHERE l.completed = 0
        AND l.last_watched_at IS NOT NULL
        AND l.progress_seconds > 5
      ORDER BY l.last_watched_at DESC
      LIMIT $1`,
    [limit],
  );
}

/** Recently opened files, newest first, in the palette's shape. */
export async function getRecentlyAccessedFiles(limit = 8): Promise<LibraryFileHit[]> {
  const db = await getDb();
  return db.select<LibraryFileHit[]>(
    `SELECT f.*, s.code AS subject_code
       FROM files f
       JOIN subjects s ON s.id = f.subject_id
      WHERE f.last_accessed_at IS NOT NULL
      ORDER BY f.last_accessed_at DESC
      LIMIT $1`,
    [limit],
  );
}

// ── Local calendar events ────────────────────────────────────────────────────

/** A calendar row the user wrote. Kept out of `calendar_events`, which every
 *  sync replaces. `subject_code` is NULL for a personal event. */
export interface DbLocalEvent {
  id: number;
  subject_id: number | null;
  subject_code: string | null;
  kind: string;              // 'due' | 'class' | 'note'
  title: string;
  start_at: string;          // ISO8601
  end_at: string | null;
  all_day: number;
  notes: string | null;
  source: string;            // 'manual' (old rows may say 'automation')
  created_at: string;
}

/** Every local event, oldest first (unwindowed, like `getCalendarEvents`). */
export async function getLocalEvents(): Promise<DbLocalEvent[]> {
  const db = await getDb();
  return db.select<DbLocalEvent[]>(
    `SELECT le.id, le.subject_id, s.code AS subject_code, le.kind, le.title,
            le.start_at, le.end_at, le.all_day, le.notes, le.source, le.created_at
       FROM local_events le
       LEFT JOIN subjects s ON s.id = le.subject_id
      ORDER BY le.start_at ASC`,
  );
}

/** A local event as the editor holds it. `subjectId` null = "Personal";
 *  dates are ISO 8601 instants. */
export interface LocalEventInput {
  subjectId: number | null;
  /** `note`, `class` or `due`. */
  kind: string;
  title: string;
  startAt: string;
  endAt: string | null;
  allDay: boolean;
  notes: string | null;
}

/** Write a local event and return its id (from `execute()`, for the pooled-
 *  connection reason in `startSyncRun`). */
export async function createLocalEvent(input: LocalEventInput): Promise<number> {
  const db = await getDb();
  const res = await db.execute(
    `INSERT INTO local_events
       (subject_id, kind, title, start_at, end_at, all_day, notes, source)
     VALUES ($1, $2, $3, $4, $5, $6, $7, 'manual')`,
    [
      input.subjectId,
      input.kind,
      input.title,
      input.startAt,
      input.endAt,
      input.allDay ? 1 : 0,
      input.notes,
    ],
  );
  if (res.lastInsertId == null) throw new Error("local event insert returned no id");
  return res.lastInsertId;
}

/** Rewrite every editable column. `source` is left as it was. */
export async function updateLocalEvent(
  id: number,
  input: LocalEventInput,
): Promise<void> {
  const db = await getDb();
  await db.execute(
    `UPDATE local_events
        SET subject_id = $1, kind = $2, title = $3, start_at = $4,
            end_at = $5, all_day = $6, notes = $7
      WHERE id = $8`,
    [
      input.subjectId,
      input.kind,
      input.title,
      input.startAt,
      input.endAt,
      input.allDay ? 1 : 0,
      input.notes,
      id,
    ],
  );
}

export async function deleteLocalEvent(id: number): Promise<void> {
  const db = await getDb();
  await db.execute(`DELETE FROM local_events WHERE id = $1`, [id]);
}
