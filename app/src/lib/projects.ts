import { getDb, matchSql, type SearchScope } from "@/lib/db";
import { patchColumns } from "@/lib/sqlPatch";

/**
 * Projects: a piece of work scoped to one subject or none, broken into tasks
 * and one level of subtask. Direct SQL over `getDb()`; the `oculus` CLI writes
 * the same rows through `app/src-tauri/src/projects.rs`, so a change to a
 * table's shape moves both writers together.
 *
 * A task with `project_id` NULL is *unfiled* — it belongs to no project; its
 * board is {@link boardOf}'s default.
 */

// ── Columns ──────────────────────────────────────────────────────────────────

/** What a board column means (the name is the user's): `done` stamps
 *  `done_at`, `backlog` is the one the board can leave out. */
export type ColumnKind = "backlog" | "active" | "done";

export interface ProjectColumn {
  id: string;
  name: string;
  kind: ColumnKind;
}

/** The board a new project opens with; only `kind` is load-bearing. Frozen
 *  because it is shared — mutate a {@link freshColumns} copy instead. */
export const DEFAULT_COLUMNS: readonly ProjectColumn[] = Object.freeze([
  Object.freeze({ id: "backlog", name: "Backlog", kind: "backlog" }),
  Object.freeze({ id: "todo", name: "Todo", kind: "active" }),
  Object.freeze({ id: "doing", name: "In progress", kind: "active" }),
  Object.freeze({ id: "done", name: "Done", kind: "done" }),
] as ProjectColumn[]);

function freshColumns(): ProjectColumn[] {
  return DEFAULT_COLUMNS.map((c) => ({ ...c }));
}

/**
 * The board a task's column is checked and drawn against: the project's own,
 * or the default for an unfiled task (whose `column_id` is still NOT NULL and
 * names a default column id). Mirrors `board_of` in `app/src-tauri/src/projects.rs`.
 */
export function boardOf(project: DbProject | null | undefined): ProjectColumn[] {
  return project?.columns ?? freshColumns();
}

// ── Rows ─────────────────────────────────────────────────────────────────────

/** A project row plus its subject's code (joined, never stored).
 *  `subject_id` NULL is a personal project. */
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
  /** Parsed from the stored JSON. */
  columns: ProjectColumn[];
  /** Parsed from JSON; `[]` when untagged, never `null`. */
  tags: string[];
  /**
   * A `CalEvent.id` as `app/src/lib/calendar.ts` mints it. Resolved live, not a
   * foreign key — a sync re-inserts Canvas rows — so a stale id draws nothing.
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
  /** `null` on an unfiled task. */
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
  /** Set while the task sits in a `kind: "done"` column; `moveTask` owns it. */
  done_at: string | null;
  source: string;
  created_at: string;
  updated_at: string;
}

/** A task with enough of its project to draw and open it. The project
 *  fields are all `null` on an unfiled task. */
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

/** The stored JSON tags, or `[]`; non-strings are dropped. */
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
    // Fall back rather than throw: one bad row mustn't take the list down.
    columns = freshColumns();
  }
  return { ...row, columns, tags: parseTags(row.tags) };
}

/** Tags as stored: trimmed, deduplicated case-insensitively (first spelling
 *  wins) and capped — normalised on write so reads can compare strings. */
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

const MAX_TAGS = 24;

// ── Change notification ──────────────────────────────────────────────────────

/** Fired after any write here so open boards re-read — a window event,
 *  since these writes start in the frontend. */
export const PROJECTS_UPDATED_EVENT = "oculus:projects-updated";

export function notifyProjectsUpdated(): void {
  window.dispatchEvent(new CustomEvent(PROJECTS_UPDATED_EVENT));
}

// ── Reads ────────────────────────────────────────────────────────────────────

export interface GetProjectsOptions {
  /** One subject; `null` for personal projects. Omit for all. */
  subjectId?: number | null;
  /** Defaults to 'active'; `"all"` for both. */
  status?: string;
}

/** Every project, in board `position` order. */
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

export interface ProjectHit {
  id: number;
  name: string;
  subject_code: string | null;
  /** 'active' | 'archived' — archived projects stay findable. */
  status: string;
  due_at: string | null;
}

/** A task as a search result; `project_id` null is an unfiled task. */
export interface TaskHit {
  id: number;
  title: string;
  project_id: number | null;
  project_name: string | null;
  subject_code: string | null;
  done_at: string | null;
  due_at: string | null;
}

/** Projects matching a search query (`matchSql`'s rule), best first; active
 *  before archived, then soonest due. */
export async function searchProjects(
  query: string,
  limit = 4,
  scope: Pick<SearchScope, "subjectId"> = {},
): Promise<ProjectHit[]> {
  const db = await getDb();
  const { where, rank, params } = matchSql(
    `p.name || ' ' || COALESCE(s.code, '')`,
    query,
    [["p.subject_id", scope.subjectId]],
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
 * Tasks matching a search query, with the project's name in the haystack;
 * unfinished first. The project join is LEFT and its name COALESCEd, so
 * unfiled tasks match (`||` with NULL is NULL).
 */
export async function searchTasks(
  query: string,
  limit = 4,
  scope: Pick<SearchScope, "subjectId"> = {},
): Promise<TaskHit[]> {
  const db = await getDb();
  // A subject scope is the project's, so it drops unfiled tasks.
  const { where, rank, params } = matchSql(
    `t.title || ' ' || COALESCE(p.name, '')`,
    query,
    [["p.subject_id", scope.subjectId]],
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

/** `null` if it has been deleted. */
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
 * Every task of one project, parents and subtasks, by `position`. The caller
 * groups by `column_id`; don't add it to ORDER BY — that sorts columns
 * alphabetically, not in the board's order.
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

/** Cross-project reads join the project with LEFT joins, so unfiled tasks
 *  (no project row) are kept. */
const WITH_PROJECT = `SELECT t.*, p.name AS project_name, p.subject_id AS project_subject_id,
            s.code AS project_subject_code
       FROM project_tasks t
       LEFT JOIN projects p ON p.id = t.project_id
       LEFT JOIN subjects s ON s.id = p.subject_id`;

/** Dated, unfinished tasks across every project — the calendar's task layer. */
export async function getAllOpenTasks(): Promise<DbOpenTask[]> {
  const db = await getDb();
  return db.select<DbOpenTask[]>(
    `${WITH_PROJECT}
      WHERE t.due_at IS NOT NULL AND t.done_at IS NULL
      ORDER BY t.due_at ASC`,
  );
}

/** Order for lists that span projects. `position` only compares within one
 *  project's column, so sort by due date, then project (unfiled first), and
 *  only then `position`. */
const UNIVERSAL_ORDER = `ORDER BY t.due_at IS NULL, t.due_at ASC,
               t.project_id IS NOT NULL, t.project_id ASC,
               t.position ASC, t.id ASC`;

/** Every unfiled task, with the (null) project columns still selected. */
export async function getUnfiledTasks(): Promise<DbTaskWithProject[]> {
  const db = await getDb();
  return db.select<DbTaskWithProject[]>(
    `${WITH_PROJECT}
      WHERE t.project_id IS NULL
      ${UNIVERSAL_ORDER}`,
  );
}

/** Every task in the library, finished ones included. */
export async function getAllTasks(): Promise<DbTaskWithProject[]> {
  const db = await getDb();
  return db.select<DbTaskWithProject[]>(
    `${WITH_PROJECT}
      ${UNIVERSAL_ORDER}`,
  );
}

export interface ProjectTaskCounts {
  /** Subtasks included — must match `task_counts` in
   *  `app/src-tauri/src/projects.rs`. */
  total: number;
  /** Counted off `done_at`. */
  done: number;
}

/** Finished/total for many projects in one `GROUP BY`. Every id asked for is
 *  seeded at 0/0, since a project with no tasks has no group. */
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
  /** `null` or omitted: a personal project. */
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

/** Create a project at the end of the list and return its id (from
 *  `execute()` — see `startSyncRun`). */
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
  /** Replaces the whole set, normalised ({@link normaliseTags}). */
  tags?: string[];
  /** `null` unpins the project from its event. */
  eventId?: string | null;
  position?: number;
}

/** Patch a project: `undefined` leaves a field alone, `null` clears it. */
export async function updateProject(id: number, patch: UpdateProjectInput): Promise<void> {
  const db = await getDb();
  const changed = await patchColumns(db, "projects", id, {
    name: patch.name,
    subject_id: patch.subjectId,
    brief: patch.brief,
    status: patch.status,
    starts_at: patch.startsAt,
    due_at: patch.dueAt,
    columns: patch.columns === undefined ? undefined : JSON.stringify(patch.columns),
    tags: patch.tags === undefined ? undefined : JSON.stringify(normaliseTags(patch.tags)),
    event_id: patch.eventId,
    position: patch.position,
  });
  if (!changed) return;
  notifyProjectsUpdated();
}

/** Archiving is a status, not a delete — see {@link deleteProject}. */
export async function archiveProject(id: number): Promise<void> {
  await updateProject(id, { status: "archived" });
}

export async function unarchiveProject(id: number): Promise<void> {
  await updateProject(id, { status: "active" });
}

/** Every tag in use across all projects, most used first; deduplicated
 *  case-insensitively like {@link normaliseTags}. */
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

/** Deletes the project and, by cascade, every task under it. */
export async function deleteProject(id: number): Promise<void> {
  const db = await getDb();
  await db.execute(`DELETE FROM projects WHERE id = $1`, [id]);
  notifyProjectsUpdated();
}

// ── Task writes ──────────────────────────────────────────────────────────────

/**
 * Resolve a column id against the task's board ({@link boardOf}), or throw. A
 * task in a column its board lacks is drawn nowhere, and the CLI's `--column`
 * is free text, so placement is always checked.
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

/** Bump the project's `updated_at` on any task write, as the Rust writers
 *  do. `null` (unfiled, or gone) is a no-op. */
async function touchProject(projectId: number | null): Promise<void> {
  if (projectId == null) return;
  const db = await getDb();
  await db.execute(
    `UPDATE projects SET updated_at = datetime('now') WHERE id = $1`,
    [projectId],
  );
}

/** A task's project, or `null` if it is unfiled or gone. */
async function projectOfTask(id: number): Promise<number | null> {
  const db = await getDb();
  const rows = await db.select<{ project_id: number | null }[]>(
    `SELECT project_id FROM project_tasks WHERE id = $1`,
    [id],
  );
  return rows.length ? rows[0].project_id : null;
}

export interface CreateTaskInput {
  /** `null` leaves the task unfiled. */
  projectId: number | null;
  title: string;
  /** Makes this a subtask; one level only — see {@link assertCanParent}. */
  parentId?: number | null;
  body?: string | null;
  /** Defaults to the parent's column for a subtask, else the board's first. */
  columnId?: string;
  startsAt?: string | null;
  dueAt?: string | null;
  estimateMinutes?: number | null;
  source?: string;
}

/**
 * Subtasks are one level deep and share their parent's project ("no project"
 * included). Enforced here, as in `assert_can_parent` in
 * `app/src-tauri/src/projects.rs`, because SQLite can't express it and the
 * views draw a task and its children, not a tree. Returns the parent's column.
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
 * Create a task (or subtask) at the end of its column and return its id. The
 * column is checked ({@link requireColumn}); a task created into a `done`
 * column is born finished, as {@link moveTask} would make it.
 *
 * A subtask with no explicit column inherits its parent's, not the board's
 * first (Backlog), which views that list top-level tasks would never draw.
 * Mirrors `create_tasks` in `app/src-tauri/src/projects.rs`.
 */
export async function createTask(input: CreateTaskInput): Promise<number> {
  const db = await getDb();
  const parentColumnId =
    input.parentId != null
      ? await assertCanParent(input.parentId, input.projectId ?? null)
      : null;

  const project = input.projectId != null ? await getProject(input.projectId) : null;
  if (input.projectId != null && !project) {
    throw new Error(`project ${input.projectId} does not exist`);
  }
  const board = boardOf(project);
  const column =
    input.columnId !== undefined
      ? requireColumn(project, input.columnId)
      : // A parent column the board has since dropped falls back to the first.
        board.find((c) => c.id === parentColumnId) ?? board[0];
  const columnId = column.id;

  // `IS`, not `=`: `project_id = NULL` matches nothing, and unfiled tasks are a group.
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
 * What a task patch may touch — everything except where the task sits.
 * `column_id`, `position` and `done_at` are one fact written only by
 * {@link moveTask}, which reads the board to know whether the destination is
 * a `done` column; a plain `column_id` write would leave `done_at` wrong.
 */
export interface UpdateTaskInput {
  title?: string;
  body?: string | null;
  parentId?: number | null;
  startsAt?: string | null;
  dueAt?: string | null;
  estimateMinutes?: number | null;
}

/** Patch a task (`undefined` leaves alone, `null` clears). Re-parenting is
 *  checked both ways; placement goes through {@link moveTask}. */
export async function updateTask(id: number, patch: UpdateTaskInput): Promise<void> {
  const db = await getDb();
  if (patch.parentId != null) {
    if (patch.parentId === id) throw new Error("a task cannot be its own parent");
    await assertCanParent(patch.parentId, await projectOfTask(id));
    if (await hasChildren(id)) {
      throw new Error("subtasks are one level deep: a task with children cannot have a parent");
    }
  }
  const changed = await patchColumns(db, "project_tasks", id, {
    title: patch.title,
    body: patch.body,
    parent_id: patch.parentId,
    starts_at: patch.startsAt,
    due_at: patch.dueAt,
    estimate_minutes: patch.estimateMinutes,
  });
  if (!changed) return;
  await touchProject(await projectOfTask(id));
  notifyProjectsUpdated();
}

/** Deletes the task and, by cascade, its subtasks. */
export async function deleteTask(id: number): Promise<void> {
  const db = await getDb();
  const projectId = await projectOfTask(id);
  await db.execute(`DELETE FROM project_tasks WHERE id = $1`, [id]);
  await touchProject(projectId);
  notifyProjectsUpdated();
}

/** Below this gap repeated midpoints lose double precision, so the column is
 *  renumbered. */
const MIN_GAP = 1e-6;

async function renumberColumn(projectId: number | null, columnId: string): Promise<void> {
  const db = await getDb();
  // `IS`, not `=`, so unfiled tasks form a group.
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
 * Drop a task into a column between `beforeId` (above) and `afterId` (below),
 * either nullable. `position REAL` takes their midpoint so a drag writes one
 * row; when the gap underflows ({@link MIN_GAP}) the column is renumbered and
 * the midpoint retaken. The destination's *kind* sets or clears `done_at`.
 * Unfiled tasks move among the unfiled tasks of that column.
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

/** A column's kind on its own board, `"backlog"` if the board lacks it. Must
 *  agree with `universalColumnOf` (components/projects/universalTasks.ts) and
 *  `kind_of` in `app/src-tauri/src/projects.rs`. */
function kindOf(project: DbProject | null, columnId: string): ColumnKind {
  return boardOf(project).find((c) => c.id === columnId)?.kind ?? "backlog";
}

/**
 * File a task under another project, or none, taking its subtasks with it — the
 * only writer of `project_id` after creation. Refuses a lone subtask (it must
 * stay with its parent).
 *
 * Columns map across by *kind*, never id: each row lands at the end of the
 * destination's first column of that kind, and a board with no such kind is
 * refused. `done_at` is re-derived from the kind, as {@link moveTask} does.
 * Positions use `project_id IS $1` because `= NULL` matches nothing.
 * Mirrors `refile_task` in `app/src-tauri/src/projects.rs`.
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
  // Already there: re-appending would move it to the end of its column.
  if (existing.project_id === projectId) return;

  const source = existing.project_id != null ? await getProject(existing.project_id) : null;
  if (existing.project_id != null && !source) {
    throw new Error(`project ${existing.project_id} does not exist`);
  }
  const target = projectId != null ? await getProject(projectId) : null;
  if (projectId != null && !target) throw new Error(`project ${projectId} does not exist`);

  // Parent first, so it takes the lower position in a shared column.
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

  await touchProject(existing.project_id);
  await touchProject(projectId);
  notifyProjectsUpdated();
}
