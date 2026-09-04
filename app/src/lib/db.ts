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

// ── LLM settings ─────────────────────────────────────────────────────────────

export type LlmProviderKind =
  | "ollama"
  | "lmstudio"
  | "openrouter"
  | "opencode-go"
  | "custom";

/** How many models the fallback chain holds — mirrors `MAX_FALLBACKS` in
 *  `app/src-tauri/src/llm.rs`. Adding past it drops the last one. */
export const MAX_FALLBACKS = 5;

/** One configured endpoint. `id` is generated once and never changes: it is
 *  the keychain account holding that provider's key. */
export interface LlmProvider {
  id: string;
  kind: LlmProviderKind;
  label: string;
  /** Overrides the kind's default base URL; required for `custom`. */
  baseUrl: string | null;
}

/** A model in the library. Provider and model id together — the same model id
 *  can be served by two providers. */
export interface ModelRef {
  providerId: string;
  model: string;
}

/** Mirrors `LlmConfig` in `app/src-tauri/src/llm.rs` (serde camelCase) — Rust
 *  reads the same JSON headlessly to make model calls. API keys are NOT here:
 *  they live in the macOS keychain, reachable only through the llm_* commands. */
export interface LlmSettings {
  providers: LlmProvider[];
  /** The curated models; every picker in the app chooses from this list. */
  library: ModelRef[];
  chatModel: ModelRef | null;
  summaryModel: ModelRef | null;
  /** Tried in order when the chosen model cannot run. */
  fallbacks: ModelRef[];
  limits: {
    monthlyUsd: number | null;
    monthlyTokens: number | null;
  };
}

export const DEFAULT_LLM_SETTINGS: LlmSettings = {
  providers: [],
  library: [],
  chatModel: null,
  summaryModel: null,
  fallbacks: [],
  limits: { monthlyUsd: null, monthlyTokens: null },
};

const LLM_SETTINGS_KEY = "llm";

/** Every kind the client understands, in the order the Add dialog lists them.
 *  `addable: false` keeps a kind rendering and resolving for a config that
 *  already names it without offering it to new ones. */
export const PROVIDER_KINDS: {
  kind: LlmProviderKind;
  label: string;
  needsKey: boolean;
  addable: boolean;
}[] = [
  { kind: "opencode-go", label: "OpenCode Go", needsKey: true, addable: true },
  { kind: "openrouter", label: "OpenRouter", needsKey: true, addable: true },
  { kind: "ollama", label: "Ollama", needsKey: false, addable: true },
  { kind: "custom", label: "Custom", needsKey: true, addable: true },
  { kind: "lmstudio", label: "LM Studio", needsKey: false, addable: false },
];

/** What Add provider offers: three presets worth having, then Custom for
 *  everything else. LM Studio is not among them — it is Custom with a
 *  localhost URL, and a short list is the point. */
export const ADDABLE_PROVIDER_KINDS = PROVIDER_KINDS.filter((p) => p.addable);

export const providerNeedsKey = (kind: LlmProviderKind) =>
  PROVIDER_KINDS.find((p) => p.kind === kind)?.needsKey ?? true;

/** Stable string form of a model ref, for React keys and `<Select>` values. */
// The separator is a literal NUL, written as an escape: a raw one in the
// source makes every grep treat this file as binary.
export const modelKey = (m: ModelRef) => `${m.providerId}\u0000${m.model}`;
export const sameModel = (a: ModelRef | null, b: ModelRef | null) =>
  a != null && b != null && a.providerId === b.providerId && a.model === b.model;

/** Settings written before multi-provider support: one provider, three bare
 *  model names. Upgraded on read (Rust does the same in `load_config`); the
 *  first edit in Settings → AI writes the new shape back. The synthesised
 *  provider id equals the old provider name, so its keychain key still works. */
function migrateLegacy(parsed: any): LlmSettings {
  const kind: LlmProviderKind = parsed.provider ?? "ollama";
  const provider: LlmProvider = {
    id: kind,
    kind,
    label: PROVIDER_KINDS.find((p) => p.kind === kind)?.label ?? "Custom",
    baseUrl: parsed.baseUrl ?? null,
  };
  const ref = (name: unknown): ModelRef | null =>
    typeof name === "string" && name.trim()
      ? { providerId: provider.id, model: name }
      : null;
  const chatModel = ref(parsed.chatModel);
  const summaryModel = ref(parsed.summaryModel);
  const fallback = ref(parsed.fallbackModel);

  const library: ModelRef[] = [];
  for (const m of [chatModel, summaryModel, fallback]) {
    if (m && !library.some((l) => sameModel(l, m))) library.push(m);
  }

  return {
    providers: [provider],
    library,
    chatModel,
    summaryModel,
    fallbacks: fallback ? [fallback] : [],
    limits: { ...DEFAULT_LLM_SETTINGS.limits, ...(parsed.limits ?? {}) },
  };
}

export async function getLlmSettings(): Promise<LlmSettings> {
  const raw = await getSetting(LLM_SETTINGS_KEY);
  if (!raw) return structuredClone(DEFAULT_LLM_SETTINGS);
  try {
    const parsed = JSON.parse(raw);
    if (!Array.isArray(parsed.providers) || parsed.providers.length === 0) {
      return migrateLegacy(parsed);
    }
    return {
      ...DEFAULT_LLM_SETTINGS,
      ...parsed,
      limits: { ...DEFAULT_LLM_SETTINGS.limits, ...(parsed.limits ?? {}) },
    };
  } catch {
    return structuredClone(DEFAULT_LLM_SETTINGS);
  }
}

export async function setLlmSettings(settings: LlmSettings): Promise<void> {
  await setSetting(LLM_SETTINGS_KEY, JSON.stringify(settings));
}

// ── Chats ────────────────────────────────────────────────────────────────────
//
// Rows here are written by Rust (`app/src-tauri/src/agent.rs`), not by this
// module — the agent loop re-reads its own tool turns, so the history has to
// be authoritative where the loop runs. These are the read side plus delete.

export interface DbChat {
  id: number;
  title: string | null;
  created_at: string;
  updated_at: string;
}

export interface DbChatMessage {
  id: number;
  chat_id: number;
  role: "user" | "assistant" | "tool";
  content: string | null;
  tool_calls: string | null;
  tool_call_id: string | null;
  /** JSON array of {subject_id, relative_path, filename, page_no}. */
  citations: string | null;
  model: string | null;
  created_at: string;
}

export async function getChats(limit = 50): Promise<DbChat[]> {
  const db = await getDb();
  return db.select<DbChat[]>(
    `SELECT id, title, created_at, updated_at FROM chats
     ORDER BY updated_at DESC LIMIT $1`,
    [limit],
  );
}

/** Display history: the tool plumbing turns are for the model, not the reader. */
export async function getChatMessages(chatId: number): Promise<DbChatMessage[]> {
  const db = await getDb();
  return db.select<DbChatMessage[]>(
    `SELECT * FROM chat_messages
     WHERE chat_id = $1 AND role IN ('user', 'assistant') AND tool_calls IS NULL
     ORDER BY id ASC`,
    [chatId],
  );
}

export async function deleteChat(id: number): Promise<void> {
  const db = await getDb();
  await db.execute(`DELETE FROM chats WHERE id = $1`, [id]);
}

// ── Automations ──────────────────────────────────────────────────────────────

export interface DbAutomation {
  id: number;
  name: string;
  /** JSON `{nodes, links}` — parse with `parseGraph` in `lib/automations.ts`. */
  graph: string;
  enabled: boolean;
  anchor_at: string;
  last_fired_at: string | null;
  /** JSON `{ [nodeId]: { anchor, fired } }` in epoch ms — per-trigger firing
   *  state, so several triggers on one graph come due independently. Runtime
   *  state, deliberately not part of the `graph` document. */
  trigger_state: string;
  created_at: string;
  updated_at: string;
}

export async function getAutomations(): Promise<DbAutomation[]> {
  const db = await getDb();
  const rows = await db.select<any[]>(`SELECT * FROM automations ORDER BY id`);
  return rows.map((r) => ({
    ...r,
    enabled: !!r.enabled,
    trigger_state: r.trigger_state || "{}",
  }));
}

export async function addAutomation(name: string, graph: string): Promise<number> {
  const db = await getDb();
  const res = await db.execute(
    `INSERT INTO automations (name, graph) VALUES ($1, $2)`,
    [name, graph],
  );
  if (res.lastInsertId == null) throw new Error("automation insert returned no id");
  return res.lastInsertId;
}

export async function updateAutomation(
  id: number,
  fields: { name?: string; graph?: string },
): Promise<void> {
  const db = await getDb();
  if (fields.name !== undefined) {
    await db.execute(
      `UPDATE automations SET name = $1, updated_at = datetime('now') WHERE id = $2`,
      [fields.name, id],
    );
  }
  if (fields.graph !== undefined) {
    await db.execute(
      `UPDATE automations SET graph = $1, updated_at = datetime('now') WHERE id = $2`,
      [fields.graph, id],
    );
  }
}

/** Re-anchor on enable so a graph enabled now doesn't fire for a period it
 *  spent switched off — per-trigger anchors are dropped for the same reason. */
export async function setAutomationEnabled(id: number, enabled: boolean): Promise<void> {
  const db = await getDb();
  await db.execute(
    `UPDATE automations
     SET enabled = $1, updated_at = datetime('now'),
         anchor_at = CASE WHEN $1 = 1 THEN datetime('now') ELSE anchor_at END,
         trigger_state = CASE WHEN $1 = 1 THEN '{}' ELSE trigger_state END
     WHERE id = $2`,
    [enabled ? 1 : 0, id],
  );
}

/** Whole-map write of per-trigger firing state; the caller owns the merge. */
export async function setAutomationTriggerState(id: number, json: string): Promise<void> {
  const db = await getDb();
  await db.execute(`UPDATE automations SET trigger_state = $1 WHERE id = $2`, [json, id]);
}

export async function deleteAutomation(id: number): Promise<void> {
  const db = await getDb();
  await db.execute(`DELETE FROM automations WHERE id = $1`, [id]);
}

/** Display only — what stops a failing action refiring every tick is the
 *  per-trigger anchor in `trigger_state`. This must NOT touch `anchor_at`:
 *  that column is the fallback anchor for triggers which have never fired, and
 *  moving it would let one trigger's firing postpone another's first run. */
export async function markAutomationFired(id: number): Promise<void> {
  const db = await getDb();
  await db.execute(
    `UPDATE automations SET last_fired_at = datetime('now') WHERE id = $1`,
    [id],
  );
}

// ── Inbox ────────────────────────────────────────────────────────────────────

export type InboxStatus = "pending" | "ready" | "error";
export type InboxEntryStatus = "pending" | "ready" | "skipped" | "error";

export interface DbInboxItem {
  id: number;
  kind: string;
  title: string;
  run_id: number | null;
  /** The summarising instruction the automation asked for, when this item was
   *  made by a "Summarise each file" wire. Stored so a fill resumed after a
   *  quit asks the same question the graph asked. */
  instruction: string | null;
  status: InboxStatus;
  read_at: string | null;
  archived_at: string | null;
  created_at: string;
  updated_at: string;
}

export interface DbInboxEntry {
  id: number;
  item_id: number;
  subject_id: number | null;
  subject_code: string | null;
  relative_path: string;
  filename: string;
  action: string;
  status: InboxEntryStatus;
  summary_md: string | null;
  updated_at: string;
}

export async function getInboxItems(includeArchived = false): Promise<DbInboxItem[]> {
  const db = await getDb();
  return db.select<DbInboxItem[]>(
    `SELECT * FROM inbox_items
     ${includeArchived ? "" : "WHERE archived_at IS NULL"}
     ORDER BY created_at DESC`,
  );
}

export async function getInboxEntries(itemId: number): Promise<DbInboxEntry[]> {
  const db = await getDb();
  return db.select<DbInboxEntry[]>(
    `SELECT * FROM inbox_item_entries WHERE item_id = $1 ORDER BY subject_code, filename`,
    [itemId],
  );
}

export async function getUnreadInboxCount(): Promise<number> {
  const db = await getDb();
  const rows = await db.select<{ n: number }[]>(
    `SELECT COUNT(*) AS n FROM inbox_items WHERE read_at IS NULL AND archived_at IS NULL`,
  );
  return rows[0]?.n ?? 0;
}

export async function createInboxItem(
  kind: string,
  title: string,
  runId: number | null,
  instruction: string | null = null,
): Promise<number> {
  const db = await getDb();
  const res = await db.execute(
    `INSERT INTO inbox_items (kind, title, run_id, instruction) VALUES ($1, $2, $3, $4)`,
    [kind, title, runId, instruction],
  );
  if (res.lastInsertId == null) throw new Error("inbox item insert returned no id");
  return res.lastInsertId;
}

export async function addInboxEntry(
  itemId: number,
  e: {
    subjectId: number | null;
    subjectCode: string | null;
    relativePath: string;
    filename: string;
    action: string;
  },
): Promise<number> {
  const db = await getDb();
  const res = await db.execute(
    `INSERT INTO inbox_item_entries
       (item_id, subject_id, subject_code, relative_path, filename, action)
     VALUES ($1, $2, $3, $4, $5, $6)`,
    [itemId, e.subjectId, e.subjectCode, e.relativePath, e.filename, e.action],
  );
  if (res.lastInsertId == null) throw new Error("inbox entry insert returned no id");
  return res.lastInsertId;
}

/**
 * A ready-to-read Inbox item with one free-text body — what an automation's
 * "Add to Inbox" node writes.
 *
 * Reuses the digest's item/entry pair rather than adding a table: the entry
 * row already carries markdown (`summary_md`) and the Inbox already renders
 * it. A note has no file behind it, so `relative_path` is empty and `action`
 * is `note` — which is how the page knows not to draw a clickable file.
 */
export async function addInboxNote(
  title: string,
  bodyMd: string,
  runId: number | null = null,
): Promise<number> {
  const db = await getDb();
  const itemId = await createInboxItem("note", title, runId);
  await db.execute(
    `INSERT INTO inbox_item_entries
       (item_id, relative_path, filename, action, status, summary_md)
     VALUES ($1, '', $2, 'note', 'ready', $3)`,
    [itemId, title, bodyMd],
  );
  await db.execute(
    `UPDATE inbox_items SET status = 'ready', updated_at = datetime('now') WHERE id = $1`,
    [itemId],
  );
  return itemId;
}

export async function setInboxEntryResult(
  entryId: number,
  status: InboxEntryStatus,
  summaryMd: string | null,
): Promise<void> {
  const db = await getDb();
  await db.execute(
    `UPDATE inbox_item_entries
     SET status = $1, summary_md = $2, updated_at = datetime('now') WHERE id = $3`,
    [status, summaryMd, entryId],
  );
}

export async function setInboxItemStatus(id: number, status: InboxStatus): Promise<void> {
  const db = await getDb();
  await db.execute(
    `UPDATE inbox_items SET status = $1, updated_at = datetime('now') WHERE id = $2`,
    [status, id],
  );
}

export async function markInboxRead(id: number): Promise<void> {
  const db = await getDb();
  await db.execute(
    `UPDATE inbox_items SET read_at = COALESCE(read_at, datetime('now')) WHERE id = $1`,
    [id],
  );
}

export async function archiveInboxItem(id: number): Promise<void> {
  const db = await getDb();
  await db.execute(
    `UPDATE inbox_items
     SET archived_at = datetime('now'), read_at = COALESCE(read_at, datetime('now'))
     WHERE id = $1`,
    [id],
  );
}

/** Items whose per-file summaries never finished — resumed at app start. */
export async function getPendingInboxItems(): Promise<DbInboxItem[]> {
  const db = await getDb();
  return db.select<DbInboxItem[]>(
    `SELECT * FROM inbox_items WHERE status = 'pending' ORDER BY created_at`,
  );
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

// ── Sync schedules ───────────────────────────────────────────────────────────

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

export async function getFilesForSubject(subjectId: number): Promise<DbFile[]> {
  const db = await getDb();
  return db.select<DbFile[]>(
    `SELECT * FROM files WHERE subject_id = $1 ORDER BY relative_path ASC`,
    [subjectId]
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

// ── Local calendar events ────────────────────────────────────────────────────

/**
 * A calendar row Oculus wrote itself — an automation deriving a deadline, or
 * the user pinning a reminder.
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
  source: string;            // 'automation' | 'manual'
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

export async function addLocalEvent(e: {
  subjectId: number | null;
  kind: string;
  title: string;
  startAt: string;           // ISO8601
  endAt?: string | null;
  allDay?: boolean;
  notes?: string | null;
  source?: string;           // default 'automation'
}): Promise<number> {
  const db = await getDb();
  const res = await db.execute(
    `INSERT INTO local_events
       (subject_id, kind, title, start_at, end_at, all_day, notes, source)
     VALUES ($1, $2, $3, $4, $5, $6, $7, $8)`,
    [
      e.subjectId,
      e.kind,
      e.title,
      e.startAt,
      e.endAt ?? null,
      e.allDay ? 1 : 0,
      e.notes ?? null,
      e.source ?? "automation",
    ],
  );
  if (res.lastInsertId == null) throw new Error("local event insert returned no id");
  return res.lastInsertId;
}

/** Local events are user data that nothing else ever cleans up — no sync
 *  replaces them — so removing one is always an explicit act. */
export async function deleteLocalEvent(id: number): Promise<void> {
  const db = await getDb();
  await db.execute(`DELETE FROM local_events WHERE id = $1`, [id]);
}

// ── Automation sources ───────────────────────────────────────────────────────

/** Files for the Read Files node. `days` counts back from now against
 *  scraped_at; subjectIds [] means every subject. */
export async function getFilesForAutomation(opts: {
  days: number;
  subjectIds: number[];
  category: string | null;
  limit: number;
}): Promise<Array<{
  subject_id: number; subject_code: string | null;
  relative_path: string; filename: string; category: string | null;
}>> {
  const db = await getDb();
  // Built positionally rather than interpolated: subject ids come from a saved
  // graph, and a graph is a document the user edits.
  const params: unknown[] = [];
  const where: string[] = [];

  params.push(opts.days);
  where.push(`f.scraped_at >= datetime('now', '-' || $${params.length} || ' days')`);

  if (opts.subjectIds.length > 0) {
    const slots = opts.subjectIds.map((id) => {
      params.push(id);
      return `$${params.length}`;
    });
    where.push(`f.subject_id IN (${slots.join(", ")})`);
  }
  if (opts.category != null) {
    params.push(opts.category);
    where.push(`f.category = $${params.length}`);
  }
  params.push(opts.limit);

  return db.select(
    `SELECT f.subject_id, s.code AS subject_code, f.relative_path, f.filename,
            f.category
       FROM files f
       JOIN subjects s ON s.id = f.subject_id
      WHERE ${where.join(" AND ")}
      ORDER BY f.scraped_at DESC, f.filename
      LIMIT $${params.length}`,
    params,
  );
}

/** Inbox items for the Read Inbox node, newest first, with their entry text
 *  already joined so the node can hand on one block of markdown. */
export async function getInboxDigest(opts: {
  scope: "unread" | "all"; days: number; limit: number;
}): Promise<Array<{ id: number; title: string; created_at: string; body: string }>> {
  const db = await getDb();
  // The limit counts *items*, not entries, so the items are picked first and
  // their entries joined on afterwards — a LIMIT over the joined rows would
  // truncate one item's summaries mid-way.
  const rows = await db.select<
    {
      id: number;
      title: string;
      created_at: string;
      filename: string | null;
      action: string | null;
      summary_md: string | null;
    }[]
  >(
    `WITH picked AS (
       SELECT id, title, created_at
         FROM inbox_items
        WHERE archived_at IS NULL
          ${opts.scope === "unread" ? "AND read_at IS NULL" : ""}
          AND created_at >= datetime('now', '-' || $1 || ' days')
        ORDER BY created_at DESC
        LIMIT $2
     )
     SELECT p.id, p.title, p.created_at, e.filename, e.action, e.summary_md
       FROM picked p
       LEFT JOIN inbox_item_entries e ON e.item_id = p.id
      ORDER BY p.created_at DESC, e.subject_code, e.filename`,
    [opts.days, opts.limit],
  );

  const byItem = new Map<number, { id: number; title: string; created_at: string; parts: string[] }>();
  for (const r of rows) {
    let item = byItem.get(r.id);
    if (!item) {
      item = { id: r.id, title: r.title, created_at: r.created_at, parts: [] };
      byItem.set(r.id, item);
    }
    if (!r.summary_md) continue;  // pending, skipped or errored — nothing to read yet
    // A note entry is the item's whole body (`addInboxNote`), so it stands on
    // its own; a per-file summary is titled with the file it came from, or a
    // digest of six files reads as one undifferentiated wall.
    item.parts.push(
      r.action === "note" || !r.filename
        ? r.summary_md
        : `### ${r.filename}\n\n${r.summary_md}`,
    );
  }

  return [...byItem.values()].map((i) => ({
    id: i.id,
    title: i.title,
    created_at: i.created_at,
    body: i.parts.join("\n\n"),
  }));
}
