import { getDb, matchSql, type SearchScope } from "@/lib/db";
import {
  toProject,
  type DbOpenTask,
  type DbProject,
  type DbProjectTask,
  type DbTaskWithProject,
  type ProjectRow,
} from "./rows";

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
   *  `app/src-tauri/src/db/projects/read.rs`. */
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
