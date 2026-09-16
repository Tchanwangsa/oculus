import Database from "@tauri-apps/plugin-sql";

import type { Provider } from "@/lib/harness";

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

/** `scheduled` is no longer produced — only rows from the removed automations
 *  scheduler carry it — but it must still parse out of `sync_runs`. */
export type SyncOrigin = "manual" | "scheduled";

export interface SyncRun {
  id: number;
  started_at: string;
  finished_at: string | null;
  status: "running" | "completed" | "failed";
  subjects_synced: number;
  pages_scraped: number;
  error: string | null;
  /** JSON array of course codes the run targeted; NULL on pre-tracking runs. */
  subject_codes: string | null;
  /** What kicked the run off. Pre-tracking runs default to 'manual'. */
  origin: SyncOrigin;
}

/** What a sync run's write actually did to a file on disk. */
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

/** A sync run plus its per-file ledger rolled up. Runs recorded before the
 *  ledger existed have all-zero counts. */
export interface SyncRunSummary extends SyncRun {
  new_count: number;
  updated_count: number;
  unchanged_count: number;
  file_count: number;
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
  /** NULL for files scraped before recency tracking existed — those never show
   *  as "new". Set once on first insert, untouched by re-scrapes. */
  first_seen_at: string | null;
  last_accessed_at: string | null;
  /** When a scrape last found the file's bytes new or changed — unlike
   *  scraped_at, which bumps every run. Newer than last_accessed_at ⇒ the
   *  unseen dot comes back. */
  content_changed_at: string | null;
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
        // New subjects start selected only if current; ON CONFLICT leaves the
        // stored (user-chosen) selection untouched.
        c._oculus_is_current ? 1 : 0,
      ]
    );
  }
}

export async function getSubjects(): Promise<Subject[]> {
  const db = await getDb();
  // `last_synced_at` is not stored — it is the finish time of the latest
  // completed run that targeted the subject. sync_runs is the only clock;
  // interrupted/failed runs never count. json_each skips NULL subject_codes
  // (runs from before targeting was recorded).
  const rows = await db.select<Subject[]>(
    `SELECT s.*,
            (SELECT MAX(r.finished_at)
             FROM sync_runs r, json_each(r.subject_codes) j
             WHERE r.status = 'completed' AND j.value = s.code) AS last_synced_at
     FROM subjects s
     ORDER BY s.is_current DESC, s.term_name DESC, s.name ASC`
  );
  return rows.map((r) => ({ ...r, is_current: !!r.is_current, selected: !!r.selected }));
}

export async function setSubjectSelected(id: number, selected: boolean): Promise<void> {
  const db = await getDb();
  await db.execute(`UPDATE subjects SET selected = $1 WHERE id = $2`, [selected ? 1 : 0, id]);
}

// ── Sync options ─────────────────────────────────────────────────────────────

/** What a sync fetches. Mirrors `SyncOptions` in `app/src-tauri/src/sync.rs`;
 *  passed to `scrape_content` per run. The CLI always syncs everything. */
export interface SyncOptions {
  announcements: boolean;
  /** Assignments and quizzes — one Canvas phase. */
  assignments: boolean;
  /** The modules walk: pages and files. */
  modules: boolean;
  /** Ed Discussion threads. */
  ed: boolean;
  /** Echo360 lecture *list* only — refreshed after the scrape, from the
   *  frontend (the Rust engine ignores this field). Never downloads videos. */
  lectures: boolean;
  /** Canvas calendar: class times and due dates. Like `lectures`, this is a
   *  post-scrape refresh driven from the frontend, not a Rust scrape phase. */
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

// ── Parse settings ───────────────────────────────────────────────────────────

export type ParseBackend = "local" | "cloud" | "auto";

/** Mirrors the sidecar's live `/limits` state. The memory cap is for the whole
 * sidecar process tree, not each worker independently. */
export interface ParseSettings {
  memoryCapMb: number;
  backend: ParseBackend;
}

export const DEFAULT_PARSE_SETTINGS: ParseSettings = {
  memoryCapMb: 8192,
  backend: "local",
};

const PARSE_SETTINGS_KEY = "parse";

export async function getParseSettings(): Promise<ParseSettings> {
  const raw = await getSetting(PARSE_SETTINGS_KEY);
  if (!raw) return { ...DEFAULT_PARSE_SETTINGS };
  try {
    const parsed = JSON.parse(raw);
    const cap = Number(parsed.memoryCapMb);
    return {
      memoryCapMb: Math.max(
        5120,
        Number.isSafeInteger(cap) && cap > 0 ? cap : DEFAULT_PARSE_SETTINGS.memoryCapMb,
      ),
      backend: ["local", "cloud", "auto"].includes(parsed.backend)
        ? parsed.backend
        : DEFAULT_PARSE_SETTINGS.backend,
    } as ParseSettings;
  } catch {
    return { ...DEFAULT_PARSE_SETTINGS };
  }
}

export async function setParseSettings(settings: ParseSettings): Promise<void> {
  await setSetting(PARSE_SETTINGS_KEY, JSON.stringify(settings));
}

// ── Per-job models ───────────────────────────────────────────────────────────
//
// Every model-backed job that is not a chat turn — chaptering a lecture,
// naming a thread — names its own agent, model and reasoning level, the way
// the composer does for a send. One JSON value here, read back in Rust by
// `harness::jobs` since the jobs themselves run there.

/** A job's key in the stored object. Mirrors `Job` in
 *  `app/src-tauri/src/harness/jobs.rs`; adding one is a key here, a variant
 *  there, and a row in `JOBS` below. */
export type JobId = "lectureChapters" | "lectureRecap" | "threadNaming";

/** What one job runs on. `reasoningEffort` is null only for a model that
 *  takes no level — never "whatever the agent defaults to". */
export interface JobSelection {
  provider: Provider;
  model: string;
  reasoningEffort: string | null;
}

export type JobModels = Record<JobId, JobSelection>;

/** The jobs Settings → AI lists, in the order it lists them. */
export const JOBS: { id: JobId; label: string; description: string }[] = [
  {
    id: "lectureChapters",
    label: "Lecture chapters",
    description:
      "Reads a recording's slide frames and transcript and names its topics. One long turn, eight to eleven minutes.",
  },
  {
    id: "lectureRecap",
    label: "Lecture recap",
    description:
      "Writes a short note for each visual change, using the slide frame and what was said over it.",
  },
  {
    id: "threadNaming",
    label: "Chat thread names",
    description:
      "One line naming a conversation from its first exchange, once, after the first reply.",
  },
];

/** Mirrors `default_selection` in `app/src-tauri/src/harness/jobs.rs` — both
 *  sides have to agree on what an unconfigured job runs, because either can
 *  be the one that resolves it. */
export const DEFAULT_JOB_MODELS: JobModels = {
  lectureChapters: { provider: "codex", model: "gpt-5.6-luna", reasoningEffort: "xhigh" },
  lectureRecap: { provider: "codex", model: "gpt-5.6-luna", reasoningEffort: "medium" },
  threadNaming: { provider: "claude", model: "claude-haiku-4-5", reasoningEffort: "low" },
};

const JOB_MODELS_KEY = "job_models";

/** Tolerant on read: a job whose stored row is missing, gutted or from an
 *  older build falls back to its default rather than leaving a picker with
 *  nothing selected. */
export async function getJobModels(): Promise<JobModels> {
  const raw = await getSetting(JOB_MODELS_KEY);
  if (!raw) return structuredClone(DEFAULT_JOB_MODELS);
  try {
    const parsed = JSON.parse(raw);
    const out = structuredClone(DEFAULT_JOB_MODELS);
    for (const job of JOBS) {
      const row = parsed?.[job.id];
      if (!row || typeof row.model !== "string" || !row.model.trim()) continue;
      if (row.provider !== "claude" && row.provider !== "codex") continue;
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

export async function startSyncRun(
  subjectCodes: string[] = [],
  origin: SyncOrigin = "manual",
): Promise<number> {
  const db = await getDb();
  // The id must come from execute()'s own result: a follow-up
  // `SELECT last_insert_rowid()` runs on whichever pooled connection is free
  // and can return another statement's id — which once left a run stuck
  // "running" forever while its finish targeted a row that never existed.
  const res = await db.execute(
    `INSERT INTO sync_runs (status, subject_codes, origin) VALUES ('running', $1, $2)`,
    [JSON.stringify(subjectCodes), origin],
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

/** Error stamped on runs reconciled at startup. For these, `finished_at` is
 *  the reconcile time (next app launch), NOT when the sync actually died — so
 *  a duration computed from it is meaningless and the UI must not show one. */
export const INTERRUPTED_SYNC_ERROR = "Interrupted — app closed or sync stalled";

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
         error = COALESCE(error, '${INTERRUPTED_SYNC_ERROR}')
     WHERE status = 'running' AND finished_at IS NULL`,
  );
  return stale.length;
}

/** Record one file a sync run touched. */
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
/** The most recent run, finished or not — what an event-triggered graph
 *  describes when it is run by hand from the editor and has no run of its
 *  own. */
export async function getLatestSyncRunId(): Promise<number | null> {
  const db = await getDb();
  const rows = await db.select<{ id: number }[]>(
    `SELECT id FROM sync_runs ORDER BY id DESC LIMIT 1`,
  );
  return rows[0]?.id ?? null;
}

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

/** Stamp that a scrape write actually changed this file's bytes ('new' or
 *  'updated' — never 'unchanged'). What brings the unseen dot back on rows,
 *  including a module item whose target page or file changed. */
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

/** Forget a file's parse/embed state after a re-scrape changed its bytes.
 *  The Rust side purges the on-disk artifacts (`.md`, `.pages.json`,
 *  `.emb.json`); this clears the DB's view — stored pages and both status
 *  columns — so the pipeline re-runs and search never serves stale text. */
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

export async function markFileAccessed(id: number): Promise<void> {
  const db = await getDb();
  await db.execute(`UPDATE files SET last_accessed_at = datetime('now') WHERE id = $1`, [id]);
}

/** A file the chat composer's `@` can point the agent at. */
export interface MentionFile {
  id: number;
  subject_id: number;
  subject_code: string;
  filename: string;
  /** Path from the library root, e.g. `courses/COMP30026_2026_SM2/pages/x.md`
   *  — what the composer inserts and what `oculus read` takes. */
  relative_path: string;
  category: string | null;
}

/**
 * Candidates for an `@` mention, narrowed to the chat's subject when it has
 * one.
 *
 * Only files the agent can actually read are offered: `.md` is on disk as
 * written, but a PDF or slide deck has text only once the sidecar has parsed
 * it — the same `('fast', 'quality')` predicate retrieval uses. An unparsed
 * deck in the list would be an `oculus read` that comes back empty after the
 * student picked it, which is worse than not offering it.
 *
 * Ordered by prefix match, then by what they opened recently: with no query
 * typed the list is the handful of files they were just working in.
 */
export async function searchMentionFiles(
  subjectId: number | null,
  query: string,
  limit = 8,
): Promise<MentionFile[]> {
  const db = await getDb();
  const esc = query.replace(/[%_\\]/g, (c) => `\\${c}`);
  return db.select<MentionFile[]>(
    `SELECT f.id, f.subject_id, s.code AS subject_code, f.filename,
            f.relative_path, f.category
     FROM files f
     JOIN subjects s ON s.id = f.subject_id
     WHERE (f.file_type = 'md' OR f.parse_status IN ('fast', 'quality'))
       AND ($1 IS NULL OR f.subject_id = $1)
       AND ($2 = '' OR f.filename LIKE $3 ESCAPE '\\')
     ORDER BY (f.filename LIKE $4 ESCAPE '\\') DESC,
              f.last_accessed_at DESC,
              f.filename ASC
     LIMIT $5`,
    [subjectId, query, `%${esc}%`, `${esc}%`, limit],
  );
}

/**
 * One file by its library path (`courses/<subject>/…`, what `relative_path`
 * holds and what the chat's `@` menu and `oculus read` both speak).
 *
 * The path carries the subject folder, so it is specific on its own — no
 * subject id is needed and none is asked for, which is what lets a path
 * lifted out of an agent's tool call resolve without knowing where it came
 * from. `LIMIT 1` guards the theoretical tie rather than expressing a choice.
 */
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

export async function getFilesForSubject(subjectId: number): Promise<DbFile[]> {
  const db = await getDb();
  return db.select<DbFile[]>(
    `SELECT * FROM files WHERE subject_id = $1 ORDER BY relative_path ASC`,
    [subjectId]
  );
}

// ── Palette search ───────────────────────────────────────────────────────────

/**
 * What the command palette matches a typed word against.
 *
 * Slugs are the reason this is not just `filename`: an announcement is on disk
 * as `2026-07-14-welcome-to-comp30022.md` but reads as "Welcome to COMP30022"
 * (`humanizeSlug`), so the separators are flattened to spaces and each word
 * becomes matchable on its own. The subject code rides along in the same
 * string, which is what makes "comp30026 workshop" one query rather than a
 * filter plus a query.
 */
const FILE_HAYSTACK = `replace(replace(f.filename, '-', ' '), '_', ' ') || ' ' || s.code`;
const LECTURE_HAYSTACK = `l.title || ' ' || s.code`;

/** At most this many words are honoured; the rest are noise from a pasted line. */
const MAX_TERMS = 6;

function likeEscape(s: string): string {
  return s.replace(/[%_\\]/g, (c) => `\\${c}`);
}

/** The typed words, escaped for LIKE. Empty when nothing has been typed — every
 *  row matches then, and the ordering alone decides what is worth showing. */
function terms(query: string): string[] {
  return query.trim().split(/\s+/).filter(Boolean).slice(0, MAX_TERMS).map(likeEscape);
}

/**
 * Builds `AND`-ed substring predicates plus the rank expression the two
 * searches share.
 *
 * Every word must appear *somewhere* in the haystack, in any order, so
 * "algorithms graph" finds `graph-algorithms.pdf`. The rank is whether some
 * word in the haystack *starts* with the first term — a leading space is
 * prepended so the first word counts as one — which floats a real title match
 * above an incidental substring.
 */
function matchSql(
  haystack: string,
  query: string,
): { where: string; rank: string; params: string[] } {
  const words = terms(query);
  const where = words.length
    ? words.map((_, i) => `${haystack} LIKE $${i + 1} ESCAPE '\\'`).join(" AND ")
    : "1";
  const params = [...words.map((w) => `%${w}%`), `% ${words[0] ?? ""}%`];
  const rank = `((' ' || ${haystack}) LIKE $${params.length} ESCAPE '\\')`;
  return { where, rank, params };
}

/** A file the palette can open, labelled with the subject it came from. */
export interface LibraryFileHit extends DbFile {
  subject_code: string;
}

/** A lecture the palette can open. Enough of a `Lecture` to build its route. */
export interface LibraryLectureHit {
  id: string;
  subject_id: number;
  subject_code: string;
  title: string;
  date: string;
}

/**
 * Files matching a palette query, best first.
 *
 * Unlike the chat's `@` menu this offers *every* file, parsed or not: the
 * palette opens a file for a person to read, and a PDF still awaiting the
 * sidecar renders perfectly well. Ties break towards this term's coursework and
 * then towards what was opened most recently, so an empty query is the handful
 * of files you were last in.
 */
export async function searchLibraryFiles(
  query: string,
  limit = 8,
): Promise<LibraryFileHit[]> {
  const db = await getDb();
  const { where, rank, params } = matchSql(FILE_HAYSTACK, query);
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
): Promise<LibraryLectureHit[]> {
  const db = await getDb();
  const { where, rank, params } = matchSql(LECTURE_HAYSTACK, query);
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

/** Every PDF on record, with where it got to — seeds the Sync page's pipeline
 *  table so files still awaiting a parse or embed show up as backlog. */
export interface PdfPipelineRow {
  subject_id: number;
  relative_path: string;
  parse_status: string | null;
  embed_status: string | null;
  scraped_at: string | null;
  parsed_at: string | null;
  embedded_at: string | null;
}

export async function getPdfPipelineRows(): Promise<PdfPipelineRow[]> {
  const db = await getDb();
  return db.select<PdfPipelineRow[]>(
    `SELECT subject_id, relative_path, parse_status, embed_status,
            scraped_at, parsed_at, embedded_at
     FROM files
     WHERE lower(file_type) IN ('pdf', 'pptx', 'docx', 'ppt', 'doc')
     ORDER BY relative_path ASC`,
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
  // Last-synced derives from sync_runs, so the reset clears the history too —
  // otherwise every subject would still claim a sync it no longer has.
  await db.execute("DELETE FROM sync_run_files");
  await db.execute("DELETE FROM sync_runs");
  await db.execute("UPDATE sync_log SET run_id = NULL");
  await db.execute("VACUUM");
  const rows = await db.select<{ cnt: number }[]>("SELECT COUNT(*) AS cnt FROM files");
  return rows[0]?.cnt ?? 0;
}

// ── Lectures ──────────────────────────────────────────────────────────────────

/** Which of a capture's two streams: 1 the Presenter screen, 2 the room camera. */
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
  /** When it was last watched, `datetime('now')` (UTC, no zone marker) — the
   *  same idiom as `files.last_accessed_at`, so the two recency stamps compare
   *  directly. NULL is never watched; `progress_seconds` says how far in you
   *  got and this says when, which is what Home's Continue ranks on. */
  last_watched_at: string | null;
  completed: number;
  synced_at: string;
  /** The chaptering job's state: `null` (never run), `running`, `ready`,
   *  `error` — the same vocabulary `files.parse_status` uses. */
  chapter_status: string | null;
  /** Stamped only by a terminal status. */
  chaptered_at: string | null;
  /** Why the last run failed; cleared on success. A status column cannot
   *  carry a message, and the player has to be able to say what went wrong. */
  chapter_error: string | null;
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
export const videoPathColumn = (source: SourceNum) =>
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

export async function updateLectureTranscriptPath(id: string, path: string): Promise<void> {
  const db = await getDb();
  await db.execute(`UPDATE lectures SET transcript_path = $1 WHERE id = $2`, [path, id]);
}

/** Both writers stamp `last_watched_at`: saving a position *is* the record of
 *  watching, and there is no other moment to hang it off. */
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

export async function clearLectureTranscripts(): Promise<void> {
  const db = await getDb();
  await db.execute(`UPDATE lectures SET transcript_path = NULL`);
}

// ── Lecture chapters ──────────────────────────────────────────────────────────

/**
 * One row of `lecture_chapters` (migration 29), written by the chaptering job
 * in `app/src-tauri/src/chapters.rs`.
 *
 * **There is no end.** A chapter runs until the next one starts, and the last
 * until the lecture does — one fact in one column, derived by whoever reads
 * it (`chapterSpans` in `app/src/lib/lectures.ts`).
 */
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

/** The job's state for one lecture, read on its own: the player's `lecture`
 *  prop comes from a list (or the side-panel store) that is not re-read when a
 *  run lands, so the status cannot be taken from the row it was opened with. */
export async function getChapterStatus(
  lectureId: string,
): Promise<{ chapter_status: string | null; chapter_error: string | null } | null> {
  const db = await getDb();
  const rows = await db.select<
    { chapter_status: string | null; chapter_error: string | null }[]
  >(`SELECT chapter_status, chapter_error FROM lectures WHERE id = $1`, [lectureId]);
  return rows[0] ?? null;
}

// ── Calendar ──────────────────────────────────────────────────────────────────

/** A row of `calendar_events`, joined to the subject it belongs to. Times are
 *  Canvas's ISO8601 UTC strings — parse with `new Date(...)` to get local. */
export interface DbCalendarEvent {
  id: string;
  subject_id: number;
  subject_code: string;
  /** `class` (a scheduled event) or `due` (an assignment/quiz deadline). */
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

/**
 * Swap a subject's calendar for the set Canvas just returned.
 *
 * Delete-then-insert rather than upsert, for the same reason as the CLI's
 * `store::replace_calendar_events`: a cancelled class has to disappear, and an
 * upsert would leave it behind forever. The fetch is always a whole course's
 * calendar, so nothing is lost by clearing first.
 */
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

/**
 * Every stored calendar event, newest last.
 *
 * Unwindowed on purpose: a semester of classes across a handful of subjects is
 * a few hundred rows, so the page holds the lot and moves between months
 * without touching the database again.
 */
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

/** Lecture recordings across every subject — the calendar's third layer, and
 *  the only class-time record for a course whose Canvas calendar is empty. */
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

// ── Recency (Home's "Continue where you left off") ───────────────────────────

/**
 * Lectures you are in the middle of, most recently watched first.
 *
 * Three filters make "in the middle of" mean something: not `completed`,
 * `last_watched_at` actually set (migration 32 backfilled nothing, so an old
 * row that was watched before the column existed stays out rather than
 * claiming a stamp it never had), and past the same five-second "actually
 * started" threshold `progressLabel` in `app/src/lib/lectures.ts` uses — a
 * second of a recording opened and closed again is not somewhere you left off.
 * The 5 is repeated rather than imported because it is a SQL predicate here
 * and a label's branch there; they must agree, and this comment is the link.
 *
 * Joins `subjects` for the code, the way `getAllLectures` does, so a row can be
 * labelled and routed without a second query.
 */
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

/**
 * Files you opened recently, newest first — `LibraryFileHit`, the same shape
 * the palette returns, so a row opens through `openFileSmart` with nothing
 * added.
 *
 * `last_accessed_at` is stamped by `markFileAccessed` with the same
 * `datetime('now')` as a lecture's `last_watched_at`, which is what lets Home
 * rank the two against each other.
 */
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

/**
 * A calendar row Oculus wrote itself — the user pinning a reminder or a
 * deadline the calendar has no Canvas source for.
 *
 * Separate from `calendar_events` because that table is Canvas's: every sync
 * deletes a subject's rows and re-inserts them (see `replaceCalendarEvents`),
 * so anything written there is gone by the next sync. `subject_code` is NULL
 * for an event that belongs to no subject.
 */
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
  source: string;            // 'manual' or 'automation' — see docs/calendar.md
  created_at: string;
}

/** Every local event, oldest first — the same unwindowed read as
 *  `getCalendarEvents`, and for the same reason. */
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

/** Local events are user data that nothing else ever cleans up — no sync
 *  replaces them — so removing one is always an explicit act. */
export async function deleteLocalEvent(id: number): Promise<void> {
  const db = await getDb();
  await db.execute(`DELETE FROM local_events WHERE id = $1`, [id]);
}
