import { getDb } from "@/lib/db";
import { patchColumns } from "@/lib/planning/sqlPatch";
import { boardOf, type ColumnKind, type ProjectColumn } from "./columns";
import { notifyProjectsUpdated } from "./events";
import { getProject } from "./reads";
import type { DbProject } from "./rows";

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
 * `app/src-tauri/src/db/projects/tasks/create.rs`, because SQLite can't express it and the
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
 * Mirrors `create_tasks` in `app/src-tauri/src/db/projects/tasks/create.rs`.
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
 *  agree with `universalColumnOf` (components/projects/tasks/universalTasks.ts) and
 *  `kind_of` in `app/src-tauri/src/db/projects/tasks/mover.rs`. */
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
 * Mirrors `refile_task` in `app/src-tauri/src/db/projects/tasks/refile.rs`.
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
