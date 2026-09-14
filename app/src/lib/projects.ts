import { getDb } from "@/lib/db";

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
 * Schema is migration 27 in `app/src-tauri/src/lib.rs`.
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
  position: number;
  /** 'manual' | 'agent' — who created it. */
  source: string;
  created_at: string;
  updated_at: string;
}

export interface DbProjectTask {
  id: number;
  project_id: number;
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

/** A dated, unfinished task with enough of its project attached to draw and
 *  open it — what the calendar's task layer reads. */
export interface DbOpenTask extends DbProjectTask {
  project_name: string;
  project_subject_id: number | null;
  project_subject_code: string | null;
}

/** The row as SQLite returns it, before `columns` is parsed. */
type ProjectRow = Omit<DbProject, "columns"> & { columns: string };

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
  return { ...row, columns };
}

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
            p.starts_at, p.due_at, p.columns, p.position, p.source,
            p.created_at, p.updated_at
       FROM projects p
       LEFT JOIN subjects s ON s.id = p.subject_id
      ${where.length ? `WHERE ${where.join(" AND ")}` : ""}
      ORDER BY p.position ASC, p.id ASC`,
    args,
  );
  return rows.map(toProject);
}

/** One project, or `null` if it has been deleted out from under the caller. */
export async function getProject(id: number): Promise<DbProject | null> {
  const db = await getDb();
  const rows = await db.select<ProjectRow[]>(
    `SELECT p.id, p.subject_id, s.code AS subject_code, p.name, p.brief, p.status,
            p.starts_at, p.due_at, p.columns, p.position, p.source,
            p.created_at, p.updated_at
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

/**
 * Dated, unfinished tasks across every project — the calendar's task layer.
 *
 * A task with no `due_at` has nowhere to be drawn, and a finished one is not a
 * deadline any more, so both are filtered in SQL rather than in the page. The
 * project's name and subject ride along because the calendar colours and
 * labels by subject and has no project list of its own.
 */
export async function getAllOpenTasks(): Promise<DbOpenTask[]> {
  const db = await getDb();
  return db.select<DbOpenTask[]>(
    `SELECT t.*, p.name AS project_name, p.subject_id AS project_subject_id,
            s.code AS project_subject_code
       FROM project_tasks t
       JOIN projects p ON p.id = t.project_id
       LEFT JOIN subjects s ON s.id = p.subject_id
      WHERE t.due_at IS NOT NULL AND t.done_at IS NULL
      ORDER BY t.due_at ASC`,
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
       (subject_id, name, brief, status, starts_at, due_at, columns, position, source)
     VALUES ($1, $2, $3, 'active', $4, $5, $6, $7, $8)`,
    [
      input.subjectId ?? null,
      input.name,
      input.brief ?? null,
      input.startsAt ?? null,
      input.dueAt ?? null,
      JSON.stringify(input.columns ?? freshColumns()),
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
 */
function requireColumn(project: DbProject, columnId: string): ProjectColumn {
  const column = project.columns.find((c) => c.id === columnId);
  if (!column) {
    const known = project.columns.map((c) => c.id).join(", ");
    throw new Error(`project ${project.id} has no column "${columnId}" (has: ${known})`);
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
 * they have to agree.
 */
async function touchProject(projectId: number): Promise<void> {
  const db = await getDb();
  await db.execute(
    `UPDATE projects SET updated_at = datetime('now') WHERE id = $1`,
    [projectId],
  );
}

/** The project a task belongs to, or `null` if the row has gone. */
async function projectOfTask(id: number): Promise<number | null> {
  const db = await getDb();
  const rows = await db.select<{ project_id: number }[]>(
    `SELECT project_id FROM project_tasks WHERE id = $1`,
    [id],
  );
  return rows.length ? rows[0].project_id : null;
}

export interface CreateTaskInput {
  projectId: number;
  title: string;
  /** Makes this a subtask of that task. One level only — see below. */
  parentId?: number | null;
  body?: string | null;
  /** Defaults to the project's first column. */
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
 */
async function assertCanParent(parentId: number): Promise<string> {
  const db = await getDb();
  const rows = await db.select<{ parent_id: number | null; column_id: string }[]>(
    `SELECT parent_id, column_id FROM project_tasks WHERE id = $1`,
    [parentId],
  );
  if (!rows.length) throw new Error(`parent task ${parentId} does not exist`);
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
 */
export async function createTask(input: CreateTaskInput): Promise<number> {
  const db = await getDb();
  const parentColumnId =
    input.parentId != null ? await assertCanParent(input.parentId) : null;

  const project = await getProject(input.projectId);
  if (!project) throw new Error(`project ${input.projectId} does not exist`);
  const column =
    input.columnId !== undefined
      ? requireColumn(project, input.columnId)
      : // A column the board has since dropped falls back rather than throwing:
        // the parent's row is already there either way.
        project.columns.find((c) => c.id === parentColumnId) ?? project.columns[0];
  const columnId = column.id;

  const [{ next }] = await db.select<{ next: number }[]>(
    `SELECT COALESCE(MAX(position), -1) + 1 AS next
       FROM project_tasks WHERE project_id = $1 AND column_id = $2`,
    [input.projectId, columnId],
  );

  const res = await db.execute(
    `INSERT INTO project_tasks
       (project_id, parent_id, title, body, column_id, position, starts_at, due_at,
        estimate_minutes, done_at, source)
     VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9,
             CASE WHEN $10 THEN datetime('now') ELSE NULL END, $11)`,
    [
      input.projectId,
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
  await touchProject(input.projectId);
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
    await assertCanParent(patch.parentId);
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
  const projectId = await projectOfTask(id);
  if (projectId != null) await touchProject(projectId);
  notifyProjectsUpdated();
}

/** Deletes the task and, by the migration's self-referential cascade, its
 *  subtasks. */
export async function deleteTask(id: number): Promise<void> {
  const db = await getDb();
  const projectId = await projectOfTask(id);
  await db.execute(`DELETE FROM project_tasks WHERE id = $1`, [id]);
  if (projectId != null) await touchProject(projectId);
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
async function renumberColumn(projectId: number, columnId: string): Promise<void> {
  const db = await getDb();
  const rows = await db.select<{ id: number }[]>(
    `SELECT id FROM project_tasks
      WHERE project_id = $1 AND column_id = $2
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
 */
export async function moveTask(
  id: number,
  columnId: string,
  beforeId: number | null,
  afterId: number | null,
): Promise<void> {
  const db = await getDb();
  const rows = await db.select<{ project_id: number }[]>(
    `SELECT project_id FROM project_tasks WHERE id = $1`,
    [id],
  );
  if (!rows.length) throw new Error(`task ${id} does not exist`);
  const projectId = rows[0].project_id;

  const project = await getProject(projectId);
  if (!project) throw new Error(`project ${projectId} does not exist`);
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
