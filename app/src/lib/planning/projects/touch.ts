import { getDb } from "@/lib/db";

/** Bump the project's `updated_at` on any task write, as the Rust writers
 *  do. `null` (unfiled, or gone) is a no-op. */
export async function touchProject(projectId: number | null): Promise<void> {
  if (projectId == null) return;
  const db = await getDb();
  await db.execute(
    `UPDATE projects SET updated_at = datetime('now') WHERE id = $1`,
    [projectId],
  );
}

/** A task's project, or `null` if it is unfiled or gone. */
export async function projectOfTask(id: number): Promise<number | null> {
  const db = await getDb();
  const rows = await db.select<{ project_id: number | null }[]>(
    `SELECT project_id FROM project_tasks WHERE id = $1`,
    [id],
  );
  return rows.length ? rows[0].project_id : null;
}

/** Bump a task's `updated_at` and its project's, for a write to a table that
 *  hangs off the task (its tags, its files). */
export async function touchTask(id: number): Promise<void> {
  const db = await getDb();
  await db.execute(
    `UPDATE project_tasks SET updated_at = datetime('now') WHERE id = $1`,
    [id],
  );
  await touchProject(await projectOfTask(id));
}
