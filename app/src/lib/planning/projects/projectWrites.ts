import { getDb } from "@/lib/db";
import { patchColumns } from "@/lib/planning/sqlPatch";
import { freshColumns, type ProjectColumn } from "./columns";
import { notifyProjectsUpdated } from "./events";
import { normaliseTags, parseTags } from "./rows";

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
