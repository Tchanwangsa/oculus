import { getDb, matchSql } from "@/lib/db";

/**
 * Projects: a piece of work — an assignment, a revision plan — scoped to one
 * subject or to none, broken into tasks and one level of subtask.
 *
 * The frontend owns these writes outright. Nothing here needs the network, the
 * keychain or a subprocess, so there is no Tauri command in the path: this is
 * direct SQL over `getDb()`, the same shape as the calendar section of
 * `app/src/lib/db.ts`. `app/src-tauri/src/projects.rs` writes the same rows
 * headlessly for the `oculus` CLI, so the chat agent can plan — the same
 * two-writer pair as `store.rs` and `db.ts` for the scrape tables, and the
 * same obligation: change a table's shape and both writers move together.
 *
 * Schema is migration 27 in `app/src-tauri/src/lib.rs`, plus migration 33
 * (`tags`, `event_id`) and migration 37, which lets a task belong to no
 * project at all — `project_id` NULL, and the absence of a project rather than
 * an "Inbox" project. Everything that reads a board for such a task goes
 * through {@link boardOf}.
 */

// ── Columns ──────────────────────────────────────────────────────────────────

/**
 * What a board column *means*, as opposed to what it is called.
 *
 * The name is the user's and changes; the kind is what the app reasons about —
 * `done` is the one that stamps `done_at`, `backlog` the one the board can
 * leave out. Stored inside the project's `columns` JSON rather than as a
 * table, because a renamable per-project list is the part of this that keeps
 * moving (see the migration).
 */
export type ColumnKind = "backlog" | "active" | "done";

export interface ProjectColumn {
  id: string;
  name: string;
  kind: ColumnKind;
}

/**
 * The board a new project opens with. Only `kind` is load-bearing; every name
 * here is the user's to change.
 *
 * Frozen, and never handed out directly: it is serialised into every new
 * project and used as the fallback board, so one caller pushing a column onto
 * it would quietly change every project created afterwards. {@link freshColumns}
 * is the copy anything mutable takes.
 */
export const DEFAULT_COLUMNS: readonly ProjectColumn[] = Object.freeze([
  Object.freeze({ id: "backlog", name: "Backlog", kind: "backlog" }),
  Object.freeze({ id: "todo", name: "Todo", kind: "active" }),
  Object.freeze({ id: "doing", name: "In progress", kind: "active" }),
  Object.freeze({ id: "done", name: "Done", kind: "done" }),
] as ProjectColumn[]);

/** A fresh, mutable copy of {@link DEFAULT_COLUMNS}. */
function freshColumns(): ProjectColumn[] {
  return DEFAULT_COLUMNS.map((c) => ({ ...c }));
}

/**
 * The board a task's column is checked and drawn against: the project's own,
 * or the default one when the task belongs to no project.
 *
 * `column_id` is NOT NULL on every task, unfiled ones included, so a task with
 * no project still has to name a column something can draw. It names one of
 * {@link DEFAULT_COLUMNS}' four ids — which are also the ids a *new* project is
 * born with, so filing an unfiled task into a default board later needs no
 * translation. `board_of` in `app/src-tauri/src/projects.rs` is the same helper
 * headless; {@link requireColumn}, {@link moveTask}, {@link createTask} and
 * `StatusPill` all read the board through here rather than off a project that
 * might not exist.
 */
export function boardOf(project: DbProject | null | undefined): ProjectColumn[] {
  return project?.columns ?? freshColumns();
}

// ── Rows ─────────────────────────────────────────────────────────────────────

/**
 * A project as the app sees it: the row, plus the subject's code resolved
 * through a join.
 *
 * The code is never stored — a renamed subject would leave a copy stale, the
 * same reason `getCalendarEvents` and `harness/store.rs` join for it.
 * `subject_id` NULL (and so `subject_code` NULL) is the personal project.
 */
export interface DbProject {
  id: number;
  subject_id: number | null;
  subject_code: string | null;
  name: string;
  brief: string | null;
  /** 'active' | 'archived'. */
  status: string;
  starts_at: string | null;
  due_at: string | null;
  /** Parsed out of the stored JSON at the boundary — callers never see text. */
  columns: ProjectColumn[];
  /**
   * The user's own labels for this project, parsed out of JSON at the boundary
   * the way {@link columns} is. Always an array — `[]` is untagged, never
   * `null`, so nothing downstream has to spell the empty case twice.
   */
  tags: string[];
  /**
   * The calendar event this project answers to — a `CalEvent.id` as
   * `app/src/lib/calendar.ts` mints it, so a Canvas deadline is its Canvas id
   * and a local row is `local_<n>`.
   *
   * Resolved live against `loadCalendar()` rather than joined: a sync deletes
   * and re-inserts a subject's Canvas rows, so nothing here can be a foreign
   * key (see migration 33). An id that no longer resolves simply draws
   * nothing.
   */
  event_id: string | null;
  position: number;
  /** 'manual' | 'agent' — who created it. */
  source: string;
  created_at: string;
  updated_at: string;
}

export interface DbProjectTask {
  id: number;
  /** `null` on an unfiled task — one that belongs to no project at all
   *  (migration 37), rather than to a project called Inbox. Its board is
   *  {@link boardOf}'s fallback. */
  project_id: number | null;
  /** Non-null on a subtask. Subtasks are one level deep — see `createTask`. */
  parent_id: number | null;
  title: string;
  body: string | null;
  column_id: string;
  position: number;
  starts_at: string | null;
  due_at: string | null;
  estimate_minutes: number | null;
  /** Set when the task lands in a `kind: "done"` column, cleared when it
   *  leaves one. `moveTask` owns this. */
  done_at: string | null;
  source: string;
  created_at: string;
  updated_at: string;
}

/**
 * A task with enough of its project attached to draw and open it — the shape
 * every cross-project read hands back.
 *
 * All three project fields are nullable, and not only because a project can be
 * personal: an **unfiled** task has no project to join to at all, so the name
 * is `null` as well as the subject. Callers label such a row by the task alone.
 */
export interface DbTaskWithProject extends DbProjectTask {
  project_name: string | null;
  project_subject_id: number | null;
  project_subject_code: string | null;
}

/** A dated, unfinished task — what the calendar's task layer reads. */
export type DbOpenTask = DbTaskWithProject;

/** The row as SQLite returns it, before `columns` and `tags` are parsed. */
type ProjectRow = Omit<DbProject, "columns" | "tags"> & {
  columns: string;
  tags: string | null;
};

/** The stored JSON array of tags, or `[]` — never a throw and never a `null`.
 *  Non-strings are dropped rather than rendered as `[object Object]`. */
function parseTags(raw: string | null): string[] {
  if (!raw) return [];
  try {
    const parsed: unknown = JSON.parse(raw);
    return Array.isArray(parsed) ? parsed.filter((t): t is string => typeof t === "string") : [];
  } catch {
    return [];
  }
}

function toProject(row: ProjectRow): DbProject {
  let columns: ProjectColumn[];
  try {
    const parsed = JSON.parse(row.columns);
    columns = Array.isArray(parsed) && parsed.length ? parsed : freshColumns();
  } catch {
    // A board that will not parse is a board nothing can be dragged on, and
    // the tasks still name their column by id — so fall back rather than throw
    // and take the whole list down with one bad row.
    columns = freshColumns();
  }
  return { ...row, columns, tags: parseTags(row.tags) };
}

/**
 * Tags as they are stored: trimmed, emptied out, deduplicated
 * case-insensitively, and capped.
 *
 * Normalising on the way *in* rather than on every read is what makes "have I
 * used this tag before" a string compare everywhere else — the composer's
 * suggestions, the pill list and {@link allTags} would otherwise each need
 * their own idea of whether `Draft` and `draft` are the same label. First
 * spelling wins, so the case the user typed first is the one that sticks.
 */
export function normaliseTags(tags: string[]): string[] {
  const seen = new Set<string>();
  const out: string[] = [];
  for (const raw of tags) {
    const tag = raw.trim().replace(/\s+/g, " ");
    if (!tag) continue;
    const key = tag.toLowerCase();
    if (seen.has(key)) continue;
    seen.add(key);
    out.push(tag);
    if (out.length >= MAX_TAGS) break;
  }
  return out;
}

/** A project is labelled, not catalogued: past a couple of dozen the pills stop
 *  fitting on a row and the tag has stopped being a tag. */
const MAX_TAGS = 24;

// ── Change notification ──────────────────────────────────────────────────────

/**
 * Fired after any write here, so an open board re-reads without polling.
 *
 * A plain `window` CustomEvent rather than a Tauri event, and dispatched by
 * the writer itself, because these writes start in the frontend — the mirror
 * of `CALENDAR_UPDATED_EVENT`, which `useBackendEvents` dispatches for the
 * rows a *sync* replaced.
 */
export const PROJECTS_UPDATED_EVENT = "oculus:projects-updated";

export function notifyProjectsUpdated(): void {
  window.dispatchEvent(new CustomEvent(PROJECTS_UPDATED_EVENT));
}

// ── Reads ────────────────────────────────────────────────────────────────────

export interface GetProjectsOptions {
  /** Restrict to one subject; `null` asks for the personal ones. Omit for all. */
  subjectId?: number | null;
  /** Defaults to 'active' — an archived project is off the board until asked
   *  for by name. Pass `"all"` for both. */
  status?: string;
}

/**
 * Every project, ordered by the board's own `position`.
 *
 * Unwindowed, like `getCalendarEvents`: a student has a handful of these, and
 * the sidebar wants the lot.
 */
export async function getProjects(opts: GetProjectsOptions = {}): Promise<DbProject[]> {
  const db = await getDb();
  const where: string[] = [];
  const args: unknown[] = [];
  if (opts.subjectId !== undefined) {
    if (opts.subjectId === null) {
      where.push(`p.subject_id IS NULL`);
    } else {
      args.push(opts.subjectId);
      where.push(`p.subject_id = $${args.length}`);
    }
  }
  const status = opts.status ?? "active";
  if (status !== "all") {
    args.push(status);
    where.push(`p.status = $${args.length}`);
  }
  const rows = await db.select<ProjectRow[]>(
    `SELECT p.id, p.subject_id, s.code AS subject_code, p.name, p.brief, p.status,
            p.starts_at, p.due_at, p.columns, p.tags, p.event_id, p.position,
            p.source, p.created_at, p.updated_at
       FROM projects p
       LEFT JOIN subjects s ON s.id = p.subject_id
      ${where.length ? `WHERE ${where.join(" AND ")}` : ""}
      ORDER BY p.position ASC, p.id ASC`,
    args,
  );
  return rows.map(toProject);
}

/** A project as a search result: enough to draw a row and build its href. */
export interface ProjectHit {
  id: number;
  name: string;
  subject_code: string | null;
  /** 'active' | 'archived' — an archived project is still findable, and says
   *  so, rather than being hidden from the one place you went looking. */
  status: string;
  due_at: string | null;
}

/** A task as a search result, carrying the project it needs to be a route —
 *  or, on an unfiled task, the `null` that *is* its route (`taskHref`). */
export interface TaskHit {
  id: number;
  title: string;
  /** `null` on an unfiled task. */
  project_id: number | null;
  /** `null` on an unfiled task, which a row labels by the task alone. */
  project_name: string | null;
  subject_code: string | null;
  done_at: string | null;
  due_at: string | null;
}

/**
 * Projects matching a search query, best first — the same word-anywhere,
 * prefix-ranked rule as files and lectures (`matchSql` in `app/src/lib/db.ts`),
 * so one field searching both cannot behave differently depending on what it
 * happened to find.
 *
 * Active before archived, then soonest due: a search during semester is nearly
 * always for the thing that is still running.
 */
export async function searchProjects(
  query: string,
  limit = 4,
): Promise<ProjectHit[]> {
  const db = await getDb();
  const { where, rank, params } = matchSql(
    `p.name || ' ' || COALESCE(s.code, '')`,
    query,
  );
  return db.select<ProjectHit[]>(
    `SELECT p.id, p.name, s.code AS subject_code, p.status, p.due_at
       FROM projects p
       LEFT JOIN subjects s ON s.id = p.subject_id
      WHERE ${where}
      ORDER BY ${rank} DESC,
               (p.status = 'active') DESC,
               p.due_at IS NULL, p.due_at ASC,
               p.position ASC
      LIMIT $${params.length + 1}`,
    [...params, limit],
  );
}

/**
 * Tasks matching a search query. The project's name is part of the haystack —
 * "essay draft" should find the draft task of the essay project — and comes
 * back with the row, because a task title on its own ("Draft", "Read chapter
 * 4") names a dozen different pieces of work across a semester.
 *
 * Unfinished first: a done task is history, and history is not usually what
 * you are trying to open.
 *
 * **The project join is LEFT, and the name is COALESCEd into the haystack.**
 * An unfiled task (migration 37) has no project row, so an inner join dropped
 * it from ⌘K entirely — the one door a task with no board to find it on most
 * needs. And `||` in SQLite yields NULL if either side is NULL, so an unfiled
 * task's haystack was NULL and matched nothing even once the join was fixed:
 * the same reason `searchProjects` wraps its subject code.
 */
export async function searchTasks(
  query: string,
  limit = 4,
): Promise<TaskHit[]> {
  const db = await getDb();
  const { where, rank, params } = matchSql(
    `t.title || ' ' || COALESCE(p.name, '')`,
    query,
  );
  return db.select<TaskHit[]>(
    `SELECT t.id, t.title, t.project_id, p.name AS project_name,
            s.code AS subject_code, t.done_at, t.due_at
       FROM project_tasks t
       LEFT JOIN projects p ON p.id = t.project_id
       LEFT JOIN subjects s ON s.id = p.subject_id
      WHERE ${where}
      ORDER BY ${rank} DESC,
               t.done_at IS NOT NULL,
               t.due_at IS NULL, t.due_at ASC,
               t.position ASC
      LIMIT $${params.length + 1}`,
    [...params, limit],
  );
}

/** One project, or `null` if it has been deleted out from under the caller. */
export async function getProject(id: number): Promise<DbProject | null> {
  const db = await getDb();
  const rows = await db.select<ProjectRow[]>(
    `SELECT p.id, p.subject_id, s.code AS subject_code, p.name, p.brief, p.status,
            p.starts_at, p.due_at, p.columns, p.tags, p.event_id, p.position,
            p.source, p.created_at, p.updated_at
       FROM projects p
       LEFT JOIN subjects s ON s.id = p.subject_id
      WHERE p.id = $1`,
    [id],
  );
  return rows.length ? toProject(rows[0]) : null;
}

/**
 * Every task of one project — parents and subtasks together, in `position`
 * order.
 *
 * One query rather than one per column: a project is tens of rows, and the
 * board would otherwise fan out a query per column on every drag. Which means
 * the caller groups by `column_id` itself; there is deliberately no
 * `column_id` in the ORDER BY, because that would sort the columns
 * alphabetically — "backlog, doing, done, todo" — which is not the board's
 * order and never will be. The board's order is the `columns` array on the
 * project. Within any one column, `position` is the order.
 */
export async function getTasks(projectId: number): Promise<DbProjectTask[]> {
  const db = await getDb();
  return db.select<DbProjectTask[]>(
    `SELECT * FROM project_tasks
      WHERE project_id = $1
      ORDER BY position ASC, id ASC`,
    [projectId],
  );
}

/** The project columns every cross-project read joins for, and the joins that
 *  reach them. **LEFT**, both of them: an unfiled task has no project row, and
 *  an inner join would drop exactly the tasks these reads exist to find. */
const WITH_PROJECT = `SELECT t.*, p.name AS project_name, p.subject_id AS project_subject_id,
            s.code AS project_subject_code
       FROM project_tasks t
       LEFT JOIN projects p ON p.id = t.project_id
       LEFT JOIN subjects s ON s.id = p.subject_id`;

/**
 * Dated, unfinished tasks across every project — the calendar's task layer.
 *
 * A task with no `due_at` has nowhere to be drawn, and a finished one is not a
 * deadline any more, so both are filtered in SQL rather than in the page. The
 * project's name and subject ride along because the calendar colours and
 * labels by subject and has no project list of its own — and are `null` for an
 * unfiled task, which the calendar labels by the task alone.
 */
export async function getAllOpenTasks(): Promise<DbOpenTask[]> {
  const db = await getDb();
  return db.select<DbOpenTask[]>(
    `${WITH_PROJECT}
      WHERE t.due_at IS NOT NULL AND t.done_at IS NULL
      ORDER BY t.due_at ASC`,
  );
}

/**
 * The order a task list that spans projects is read in, and the reason it is
 * not `position`.
 *
 * `position` is only comparable **inside one project's column** — it is a
 * fractional slot in that one run of cards, so the midpoint between two
 * projects' tasks means nothing. A universal view therefore sorts by what every
 * task has: when it is due, nulls last, then which project it is on (unfiled
 * first, as the list you are expected to empty), and only then `position`,
 * which does order the tasks that do share a column.
 */
const UNIVERSAL_ORDER = `ORDER BY t.due_at IS NULL, t.due_at ASC,
               t.project_id IS NOT NULL, t.project_id ASC,
               t.position ASC, t.id ASC`;

/**
 * Every task that belongs to no project — what the universal view opens on.
 *
 * One query, with the project columns still selected (all `null` by
 * definition), so a page can hand these rows to the same components that draw
 * {@link getAllTasks}'.
 */
export async function getUnfiledTasks(): Promise<DbTaskWithProject[]> {
  const db = await getDb();
  return db.select<DbTaskWithProject[]>(
    `${WITH_PROJECT}
      WHERE t.project_id IS NULL
      ${UNIVERSAL_ORDER}`,
  );
}

/**
 * Every task in the library, filed or not, with its project's name and subject
 * where it has one.
 *
 * Unwindowed and unfiltered — including finished ones, because the universal
 * board has a Done column and the universal table is sortable by status. A
 * student has hundreds of these, not thousands, and the alternative is a query
 * per project.
 */
export async function getAllTasks(): Promise<DbTaskWithProject[]> {
  const db = await getDb();
  return db.select<DbTaskWithProject[]>(
    `${WITH_PROJECT}
      ${UNIVERSAL_ORDER}`,
  );
}

/** Finished and total tasks on one project. */
export interface ProjectTaskCounts {
  /** Every task on the project — **subtasks included**. They are work, and a
   *  breakdown whose parents alone counted would report a project as barely
   *  started while most of it was done. `task_counts` in
   *  `app/src-tauri/src/projects.rs` counts the same way; the two must agree,
   *  since `oculus project list` prints this number too. */
  total: number;
  /** Of those, the ones sitting in a `kind: "done"` column — counted off
   *  `done_at`, which {@link moveTask} and {@link createTask} are the only
   *  writers of. */
  done: number;
}

/**
 * Finished/total per project, for as many projects as you ask about, in one
 * query.
 *
 * One `GROUP BY`, not a count per project: the index and the subject tabs draw
 * a row per project and would otherwise fan out a query each, on every render
 * that follows a write. Projects with no tasks never appear in a `GROUP BY`,
 * so every id asked for is seeded at 0/0 first — a caller reading the map can
 * treat a missing key as a bug rather than as an empty project.
 */
export async function getTaskCounts(
  projectIds: number[],
): Promise<Map<number, ProjectTaskCounts>> {
  const counts = new Map<number, ProjectTaskCounts>(
    projectIds.map((id) => [id, { total: 0, done: 0 }]),
  );
  if (!projectIds.length) return counts;
  const db = await getDb();
  const placeholders = projectIds.map((_, i) => `$${i + 1}`).join(", ");
  const rows = await db.select<{ project_id: number; total: number; done: number }[]>(
    `SELECT project_id, COUNT(*) AS total, COUNT(done_at) AS done
       FROM project_tasks
      WHERE project_id IN (${placeholders})
      GROUP BY project_id`,
    projectIds,
  );
  for (const row of rows) {
    counts.set(row.project_id, { total: row.total, done: row.done });
  }
  return counts;
}

// ── Project writes ───────────────────────────────────────────────────────────

export interface CreateProjectInput {
  name: string;
  /** `null` (or omitted) is the personal project. */
  subjectId?: number | null;
  brief?: string | null;
  startsAt?: string | null;
  dueAt?: string | null;
  columns?: ProjectColumn[];
  tags?: string[];
  /** A `CalEvent.id` — see {@link DbProject.event_id}. */
  eventId?: string | null;
  source?: string;
}

/**
 * Create a project and return its id.
 *
 * New projects go to the *end* of the list: one `MAX(position) + 1` rather
 * than renumbering, the same fractional scheme the tasks use.
 *
 * The id comes from `execute()`'s own result, never a follow-up
 * `SELECT last_insert_rowid()` — that runs on whichever pooled connection is
 * free and can hand back another statement's id (see `startSyncRun`).
 */
export async function createProject(input: CreateProjectInput): Promise<number> {
  const db = await getDb();
  const [{ next }] = await db.select<{ next: number }[]>(
    `SELECT COALESCE(MAX(position), -1) + 1 AS next FROM projects`,
  );
  const res = await db.execute(
    `INSERT INTO projects
       (subject_id, name, brief, status, starts_at, due_at, columns, tags, event_id,
      position, source)
     VALUES ($1, $2, $3, 'active', $4, $5, $6, $7, $8, $9, $10)`,
    [
      input.subjectId ?? null,
      input.name,
      input.brief ?? null,
      input.startsAt ?? null,
      input.dueAt ?? null,
      JSON.stringify(input.columns ?? freshColumns()),
      JSON.stringify(normaliseTags(input.tags ?? [])),
      input.eventId ?? null,
      next,
      input.source ?? "manual",
    ],
  );
  if (res.lastInsertId == null) throw new Error("project insert returned no id");
  notifyProjectsUpdated();
  return res.lastInsertId;
}

export interface UpdateProjectInput {
  name?: string;
  subjectId?: number | null;
  brief?: string | null;
  status?: string;
  startsAt?: string | null;
  dueAt?: string | null;
  columns?: ProjectColumn[];
  /** Replaces the whole set — there is no add/remove patch, because the editor
   *  hands back the list it is showing and a partial patch would need a second
   *  read to know what it was merging into. Normalised on the way in
   *  ({@link normaliseTags}). */
  tags?: string[];
  /** A `CalEvent.id`, or `null` to unpin the project from its event. */
  eventId?: string | null;
  position?: number;
}

/**
 * Patch a project. Only the keys present are written — `undefined` means "left
 * alone", while an explicit `null` clears the column, which is how a due date
 * or a subject is taken off.
 */
export async function updateProject(id: number, patch: UpdateProjectInput): Promise<void> {
  const db = await getDb();
  const sets: string[] = [];
  const args: unknown[] = [];
  const put = (col: string, value: unknown) => {
    args.push(value);
    sets.push(`${col} = $${args.length}`);
  };
  if (patch.name !== undefined) put("name", patch.name);
  if (patch.subjectId !== undefined) put("subject_id", patch.subjectId);
  if (patch.brief !== undefined) put("brief", patch.brief);
  if (patch.status !== undefined) put("status", patch.status);
  if (patch.startsAt !== undefined) put("starts_at", patch.startsAt);
  if (patch.dueAt !== undefined) put("due_at", patch.dueAt);
  if (patch.columns !== undefined) put("columns", JSON.stringify(patch.columns));
  if (patch.tags !== undefined) put("tags", JSON.stringify(normaliseTags(patch.tags)));
  if (patch.eventId !== undefined) put("event_id", patch.eventId);
  if (patch.position !== undefined) put("position", patch.position);
  if (!sets.length) return;
  args.push(id);
  await db.execute(
    `UPDATE projects SET ${sets.join(", ")}, updated_at = datetime('now')
      WHERE id = $${args.length}`,
    args,
  );
  notifyProjectsUpdated();
}

/** Archiving is a status, not a delete: the tasks stay, and the project can
 *  come back. This is what the board's "archive" does — {@link deleteProject}
 *  is the destructive one. */
export async function archiveProject(id: number): Promise<void> {
  await updateProject(id, { status: "archived" });
}

/** The way back. Named rather than left as `updateProject(id, { status })` at
 *  each call site, so the pair reads as one reversible act and neither side
 *  has to know that "active" is the spelling. */
export async function unarchiveProject(id: number): Promise<void> {
  await updateProject(id, { status: "active" });
}

/**
 * Every tag in use, across every project, active and archived.
 *
 * Read in SQL rather than off the list a page happens to be holding: the
 * composer suggests tags you have used *anywhere*, and a subject's tab or an
 * index filtered to active projects would otherwise offer a shrinking
 * vocabulary depending on which page you were standing on. Ordered by how
 * often a tag is used, then alphabetically, so the suggestions open on the
 * labels you actually reach for. Deduplication is case-insensitive, matching
 * {@link normaliseTags}; the most-used spelling is the one offered.
 */
export async function allTags(): Promise<string[]> {
  const db = await getDb();
  const rows = await db.select<{ tags: string | null }[]>(`SELECT tags FROM projects`);
  const counts = new Map<string, { tag: string; n: number }>();
  for (const row of rows) {
    for (const tag of parseTags(row.tags)) {
      const key = tag.toLowerCase();
      const seen = counts.get(key);
      if (seen) seen.n += 1;
      else counts.set(key, { tag, n: 1 });
    }
  }
  return [...counts.values()]
    .sort((a, b) => b.n - a.n || a.tag.localeCompare(b.tag))
    .map((t) => t.tag);
}

/** Deletes the project and, by the migration's cascade, every task under it.
 *  This is user data nothing else cleans up, so it is always an explicit act —
 *  the same rule as `deleteLocalEvent`. */
export async function deleteProject(id: number): Promise<void> {
  const db = await getDb();
  await db.execute(`DELETE FROM projects WHERE id = $1`, [id]);
  notifyProjectsUpdated();
}

// ── Task writes ──────────────────────────────────────────────────────────────

/**
 * Resolve a column id against the project's board, or refuse.
 *
 * A task filed under a column the project does not have is not merely
 * misfiled — the board renders columns, so nothing draws it at all, in any
 * view. That is survivable while the only writer is a drag on a board that
 * just rendered the column; it stops being survivable at the CLI, where
 * `--column` is free text an agent typed. So the id is checked wherever a task
 * is placed ({@link createTask}, {@link moveTask}) rather than trusted.
 *
 * The board is {@link boardOf}'s, so an unfiled task is checked against the
 * default columns — a real check, not a waiver.
 */
function requireColumn(project: DbProject | null, columnId: string): ProjectColumn {
  const board = boardOf(project);
  const column = board.find((c) => c.id === columnId);
  if (!column) {
    const known = board.map((c) => c.id).join(", ");
    const whose = project ? `project ${project.id}` : "an unfiled task";
    throw new Error(`${whose} has no column "${columnId}" (has: ${known})`);
  }
  return column;
}

/**
 * Move a project's own `updated_at` when its board changes.
 *
 * A task create, update, move or delete touches the project too: a list sorted
 * by "last touched" should not call a project untouched because the change was
 * a task on it. `create_tasks`, `update_task`, `move_task` and `delete_task`
 * in `app/src-tauri/src/projects.rs` do the same — same table, two writers, so
 * they have to agree. `null` — an unfiled task, or a row that has gone — is a
 * no-op, so no call site has to spell the absence of a project twice.
 */
async function touchProject(projectId: number | null): Promise<void> {
  // An unfiled task has no project to have been touched.
  if (projectId == null) return;
  const db = await getDb();
  await db.execute(
    `UPDATE projects SET updated_at = datetime('now') WHERE id = $1`,
    [projectId],
  );
}

/** The project a task belongs to, or `null` if it is unfiled — or if the row
 *  has gone. Both answers mean the same thing to every caller: there is no
 *  project to touch. */
async function projectOfTask(id: number): Promise<number | null> {
  const db = await getDb();
  const rows = await db.select<{ project_id: number | null }[]>(
    `SELECT project_id FROM project_tasks WHERE id = $1`,
    [id],
  );
  return rows.length ? rows[0].project_id : null;
}

export interface CreateTaskInput {
  /** `null` files the task nowhere — see {@link boardOf}. */
  projectId: number | null;
  title: string;
  /** Makes this a subtask of that task. One level only — see below. */
  parentId?: number | null;
  body?: string | null;
  /** Defaults to the first column of the task's board. */
  columnId?: string;
  startsAt?: string | null;
  dueAt?: string | null;
  estimateMinutes?: number | null;
  source?: string;
}

/**
 * Subtasks are one level deep.
 *
 * Enforced here rather than in the schema — SQLite cannot express "the parent
 * has no parent" as a constraint — and enforced at all because the board and
 * the timeline draw a task and its children, not a tree: a grandchild would
 * simply never be drawn. Both directions are checked: a task cannot be filed
 * under a subtask ({@link createTask}, {@link updateTask}), and a task that
 * already has children cannot itself be given a parent ({@link updateTask}).
 *
 * The parent must also be in the **same project** — and "no project" is one of
 * the answers, since a subtask cannot sit where its parent does not. That check
 * was `assert_can_parent`'s alone in `app/src-tauri/src/projects.rs` while every
 * door here was a composer on the board the parent was drawn on; a task view
 * that spans projects can offer a parent from another one, so both writers ask.
 */
async function assertCanParent(
  parentId: number,
  projectId: number | null,
): Promise<string> {
  const db = await getDb();
  const rows = await db.select<
    { parent_id: number | null; project_id: number | null; column_id: string }[]
  >(`SELECT parent_id, project_id, column_id FROM project_tasks WHERE id = $1`, [parentId]);
  if (!rows.length) throw new Error(`parent task ${parentId} does not exist`);
  if (rows[0].project_id !== projectId) {
    throw new Error(
      rows[0].project_id == null
        ? `parent task ${parentId} belongs to no project`
        : `parent task ${parentId} belongs to project ${rows[0].project_id}`,
    );
  }
  if (rows[0].parent_id != null) {
    throw new Error("subtasks are one level deep: a subtask cannot have children");
  }
  // The column comes back because a subtask with no column of its own belongs
  // in its parent's — see {@link createTask}.
  return rows[0].column_id;
}

async function hasChildren(id: number): Promise<boolean> {
  const db = await getDb();
  const [{ n }] = await db.select<{ n: number }[]>(
    `SELECT COUNT(*) AS n FROM project_tasks WHERE parent_id = $1`,
    [id],
  );
  return n > 0;
}

/**
 * Create a task (or a subtask) at the end of its column and return its id.
 *
 * The column id is resolved against the project's own board
 * ({@link requireColumn}) rather than taken on trust, and a task created
 * straight into a `kind: "done"` column is born finished — the same rule
 * {@link moveTask} applies on a drag, so "what column is it in" and "is it
 * done" can never disagree whichever door the task came through.
 *
 * **A subtask with no column of its own inherits its parent's**, not the
 * board's first column. The first column is Backlog on a default board, and a
 * subtask of an in-progress task filed there is a row the Backlog view never
 * draws — it lists top-level tasks — while the board and the table look right,
 * because both draw a subtask under its parent wherever it claims to be. An
 * explicit `columnId` still wins: a subtask can legitimately be done while its
 * parent is not. `create_tasks` in `app/src-tauri/src/projects.rs` does the
 * same.
 *
 * `projectId: null` files the task nowhere at all, and everything above still
 * holds: the board it is checked against is {@link boardOf}'s default one, and
 * there is no project whose `updated_at` moves.
 */
export async function createTask(input: CreateTaskInput): Promise<number> {
  const db = await getDb();
  const parentColumnId =
    input.parentId != null
      ? await assertCanParent(input.parentId, input.projectId ?? null)
      : null;

  // No project is not a missing project: there is nothing to look up and
  // nothing to refuse.
  const project = input.projectId != null ? await getProject(input.projectId) : null;
  if (input.projectId != null && !project) {
    throw new Error(`project ${input.projectId} does not exist`);
  }
  const board = boardOf(project);
  const column =
    input.columnId !== undefined
      ? requireColumn(project, input.columnId)
      : // A column the board has since dropped falls back rather than throwing:
        // the parent's row is already there either way.
        board.find((c) => c.id === parentColumnId) ?? board[0];
  const columnId = column.id;

  // `IS`, not `=`: an unfiled task's neighbours are the other unfiled tasks in
  // that column, and `project_id = NULL` matches nothing at all.
  const [{ next }] = await db.select<{ next: number }[]>(
    `SELECT COALESCE(MAX(position), -1) + 1 AS next
       FROM project_tasks WHERE project_id IS $1 AND column_id = $2`,
    [input.projectId, columnId],
  );

  const res = await db.execute(
    `INSERT INTO project_tasks
       (project_id, parent_id, title, body, column_id, position, starts_at, due_at,
        estimate_minutes, done_at, source)
     VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9,
             CASE WHEN $10 THEN datetime('now') ELSE NULL END, $11)`,
    [
      input.projectId ?? null,
      input.parentId ?? null,
      input.title,
      input.body ?? null,
      columnId,
      next,
      input.startsAt ?? null,
      input.dueAt ?? null,
      input.estimateMinutes ?? null,
      column.kind === "done" ? 1 : 0,
      input.source ?? "manual",
    ],
  );
  if (res.lastInsertId == null) throw new Error("task insert returned no id");
  await touchProject(input.projectId ?? null);
  notifyProjectsUpdated();
  return res.lastInsertId;
}

/**
 * What a task patch may touch — which is everything *except* where the task
 * sits.
 *
 * `columnId`, `position` and `done_at` are deliberately absent: they are one
 * fact in three columns, and {@link moveTask} is the only thing that writes
 * them, because it is the only thing that reads the project's board to learn
 * whether the destination is a `kind: "done"` column. A plain field write of
 * `column_id` would leave `done_at` saying the opposite — a card sitting in
 * Done that the backlog and the calendar still count as outstanding. So there
 * is one door: an inline status pill, a drag, a CLI `--column`, all of them
 * call `moveTask`.
 */
export interface UpdateTaskInput {
  title?: string;
  body?: string | null;
  parentId?: number | null;
  startsAt?: string | null;
  dueAt?: string | null;
  estimateMinutes?: number | null;
}

/**
 * Patch a task, `undefined` meaning "left alone" and `null` clearing.
 *
 * Re-parenting is checked both ways here (see {@link assertCanParent}). Where
 * the task *sits* is not patchable at all — see {@link UpdateTaskInput}.
 */
export async function updateTask(id: number, patch: UpdateTaskInput): Promise<void> {
  const db = await getDb();
  if (patch.parentId != null) {
    if (patch.parentId === id) throw new Error("a task cannot be its own parent");
    await assertCanParent(patch.parentId, await projectOfTask(id));
    if (await hasChildren(id)) {
      throw new Error("subtasks are one level deep: a task with children cannot have a parent");
    }
  }
  const sets: string[] = [];
  const args: unknown[] = [];
  const put = (col: string, value: unknown) => {
    args.push(value);
    sets.push(`${col} = $${args.length}`);
  };
  if (patch.title !== undefined) put("title", patch.title);
  if (patch.body !== undefined) put("body", patch.body);
  if (patch.parentId !== undefined) put("parent_id", patch.parentId);
  if (patch.startsAt !== undefined) put("starts_at", patch.startsAt);
  if (patch.dueAt !== undefined) put("due_at", patch.dueAt);
  if (patch.estimateMinutes !== undefined) put("estimate_minutes", patch.estimateMinutes);
  if (!sets.length) return;
  args.push(id);
  await db.execute(
    `UPDATE project_tasks SET ${sets.join(", ")}, updated_at = datetime('now')
      WHERE id = $${args.length}`,
    args,
  );
  await touchProject(await projectOfTask(id));
  notifyProjectsUpdated();
}

/** Deletes the task and, by the migration's self-referential cascade, its
 *  subtasks. */
export async function deleteTask(id: number): Promise<void> {
  const db = await getDb();
  const projectId = await projectOfTask(id);
  await db.execute(`DELETE FROM project_tasks WHERE id = $1`, [id]);
  await touchProject(projectId);
  notifyProjectsUpdated();
}

/**
 * The gap at which fractional positions have to be given up on.
 *
 * Repeated midpoints halve the gap every time, so ~50 drops into the same slot
 * exhaust a double's precision; well before that the midpoint stops landing
 * strictly between its neighbours and the order goes undefined. Renumbering
 * the column on this edge is the standard guard — it is rare, and it is one
 * pass over tens of rows.
 */
const MIN_GAP = 1e-6;

/** Renumber a column to 0, 1, 2, … so fractional positions have room again. */
async function renumberColumn(projectId: number | null, columnId: string): Promise<void> {
  const db = await getDb();
  // `IS`: the unfiled tasks of one column are a group like any other.
  const rows = await db.select<{ id: number }[]>(
    `SELECT id FROM project_tasks
      WHERE project_id IS $1 AND column_id = $2
      ORDER BY position ASC, id ASC`,
    [projectId, columnId],
  );
  for (let i = 0; i < rows.length; i++) {
    await db.execute(
      `UPDATE project_tasks SET position = $1, updated_at = datetime('now') WHERE id = $2`,
      [i, rows[i].id],
    );
  }
}

async function positionOf(id: number): Promise<number | null> {
  const db = await getDb();
  const rows = await db.select<{ position: number }[]>(
    `SELECT position FROM project_tasks WHERE id = $1`,
    [id],
  );
  return rows.length ? rows[0].position : null;
}

/**
 * Drop a task into a column, between two neighbours.
 *
 * The whole point of `position REAL` (migration 27): the new position is the
 * midpoint of `beforeId` and `afterId`, so a drag writes *one* row instead of
 * renumbering everything below it. At the ends it is `first - 1` / `last + 1`,
 * and `0` in an empty column. `beforeId` is the card above the drop and
 * `afterId` the card below it; either may be null.
 *
 * The one case that is not arithmetic is the gap underflowing ({@link MIN_GAP}):
 * the column is renumbered to whole numbers and the midpoint is taken again
 * against the same neighbours, which now have room between them.
 *
 * Landing in a `kind: "done"` column stamps `done_at`; leaving one clears it.
 * That is why this reads the project's columns — the *kind* is what decides,
 * not the column's name or id, both of which are the user's to change.
 *
 * An unfiled task moves among the *unfiled* tasks of that column: `project_id
 * IS NULL` is a group like any other here, and the board whose kinds decide is
 * {@link boardOf}'s default one.
 */
export async function moveTask(
  id: number,
  columnId: string,
  beforeId: number | null,
  afterId: number | null,
): Promise<void> {
  const db = await getDb();
  const rows = await db.select<{ project_id: number | null }[]>(
    `SELECT project_id FROM project_tasks WHERE id = $1`,
    [id],
  );
  if (!rows.length) throw new Error(`task ${id} does not exist`);
  const projectId = rows[0].project_id;

  const project = projectId != null ? await getProject(projectId) : null;
  if (projectId != null && !project) {
    throw new Error(`project ${projectId} does not exist`);
  }
  const done = requireColumn(project, columnId).kind === "done";

  const midpoint = async (): Promise<number | null> => {
    const lo = beforeId != null ? await positionOf(beforeId) : null;
    const hi = afterId != null ? await positionOf(afterId) : null;
    if (lo != null && hi != null) return hi - lo < MIN_GAP ? null : (lo + hi) / 2;
    if (lo != null) return lo + 1;
    if (hi != null) return hi - 1;
    return 0;
  };

  let position = await midpoint();
  if (position == null) {
    await renumberColumn(projectId, columnId);
    position = await midpoint();
    // Two neighbours still too close after a renumber would mean the column
    // holds more rows than a double can separate, which it cannot.
    if (position == null) throw new Error("could not find a position for the task");
  }

  await db.execute(
    `UPDATE project_tasks
        SET column_id = $1,
            position  = $2,
            done_at   = CASE WHEN $3 THEN COALESCE(done_at, datetime('now')) ELSE NULL END,
            updated_at = datetime('now')
      WHERE id = $4`,
    [columnId, position, done ? 1 : 0, id],
  );
  await touchProject(projectId);
  notifyProjectsUpdated();
}

/**
 * The kind a task's column means **on its own board**, and `"backlog"` for a
 * column that board no longer has — `kindOf`'s fallback in
 * `app/src/components/projects/universalTasks.ts`, and `kind_of`'s in
 * `app/src-tauri/src/projects.rs`. All three have to agree: a refile must land
 * a card in the column the universal board just drew it in.
 */
function kindOf(project: DbProject | null, columnId: string): ColumnKind {
  return boardOf(project).find((c) => c.id === columnId)?.kind ?? "backlog";
}

/**
 * File a task under another project, or under none at all — and take its
 * subtasks with it.
 *
 * The only writer of `project_id` after a task exists, and the only operation
 * allowed to move a parent and its children at once: a subtask sits in its
 * parent's project ({@link assertCanParent} refuses both directions of the
 * alternative), so moving one side of that pair would write the very row that
 * check exists to forbid. A subtask on its own is refused and names its
 * parent — there is no honest half of this move.
 *
 * **The column maps across by *kind*, never by id.** A column id only means
 * something against the board it was checked against, and two boards share
 * nothing but what a column *means*. The destination is the **first** column
 * of that kind — `columnForKind`'s rule in
 * `app/src/components/projects/universalTasks.ts`, the same one the universal
 * board's drag writes — so entering a kind puts you at its start: a task filed
 * into a default board lands in Todo rather than skipping to In progress. A
 * board with no column of that kind is refused outright; there is no nearest
 * kind to fall back to.
 *
 * `done_at` is still derived from the destination column's kind, as
 * {@link moveTask} derives it. The kind is preserved by construction, so this
 * normally changes nothing — except in the one case it must, a task whose old
 * column its own board had dropped, which reads as `backlog` here and cannot
 * be allowed to arrive in a work column still stamped finished.
 *
 * It **appends**, at the end of the destination column, because there is no
 * slot there to aim at: `MAX(position) + 1` over `project_id IS <destination>`
 * is exactly `appendNeighbour`'s neighbour plus one, and `IS` rather than `=`
 * because filing *out* of every project is a group like any other and
 * `project_id = NULL` matches nothing. `refile_task` in
 * `app/src-tauri/src/projects.rs` is the same function headlessly; the two
 * move together.
 */
export async function refileTask(id: number, projectId: number | null): Promise<void> {
  const db = await getDb();
  const rows = await db.select<
    { project_id: number | null; parent_id: number | null; column_id: string }[]
  >(`SELECT project_id, parent_id, column_id FROM project_tasks WHERE id = $1`, [id]);
  if (!rows.length) throw new Error(`task ${id} does not exist`);
  const existing = rows[0];

  if (existing.parent_id != null) {
    throw new Error(
      `task ${id} is a subtask of task ${existing.parent_id}, and a subtask sits in its ` +
        `parent's project — refile task ${existing.parent_id} and this one travels with it`,
    );
  }
  // Already there: nothing to write, and writing anyway would append the task
  // to the end of the column it is already in.
  if (existing.project_id === projectId) return;

  const source = existing.project_id != null ? await getProject(existing.project_id) : null;
  if (existing.project_id != null && !source) {
    throw new Error(`project ${existing.project_id} does not exist`);
  }
  const target = projectId != null ? await getProject(projectId) : null;
  if (projectId != null && !target) throw new Error(`project ${projectId} does not exist`);

  // The parent first, then its children in their own order, so the parent
  // takes the lower position in any column the two end up sharing.
  const children = await db.select<{ id: number; column_id: string }[]>(
    `SELECT id, column_id FROM project_tasks
      WHERE parent_id = $1
      ORDER BY position ASC, id ASC`,
    [id],
  );
  const moving = [{ id, column_id: existing.column_id }, ...children];

  for (const row of moving) {
    const kind = kindOf(source, row.column_id);
    const column = boardOf(target).find((c) => c.kind === kind);
    if (!column) {
      const known = boardOf(target).map((c) => c.id).join(", ");
      const whose = target ? `project ${target.id}` : "an unfiled task's board";
      throw new Error(
        `${whose} has no "${kind}" column, so task ${row.id} has nowhere to land (has: ${known})`,
      );
    }
    const [{ next }] = await db.select<{ next: number }[]>(
      `SELECT COALESCE(MAX(position), -1) + 1 AS next
         FROM project_tasks WHERE project_id IS $1 AND column_id = $2`,
      [projectId, column.id],
    );
    await db.execute(
      `UPDATE project_tasks
          SET project_id = $1,
              column_id  = $2,
              position   = $3,
              done_at    = CASE WHEN $4 THEN COALESCE(done_at, datetime('now')) ELSE NULL END,
              updated_at = datetime('now')
        WHERE id = $5`,
      [projectId, column.id, next, column.kind === "done" ? 1 : 0, row.id],
    );
  }

  // Both boards changed: the one that lost the task and the one that gained
  // it. `null` is a no-op, so neither side has to spell the absence out.
  await touchProject(existing.project_id);
  await touchProject(projectId);
  notifyProjectsUpdated();
}
