import type Database from "@tauri-apps/plugin-sql";

/** Undefined leaves a column alone; null clears it. Column names are schema
 *  literals supplied by the writer, never input from an IPC or user field. */
export async function patchColumns(
  db: Pick<Database, "execute">,
  table: "projects" | "project_tasks",
  id: number,
  columns: Record<string, unknown>,
): Promise<boolean> {
  const entries = Object.entries(columns).filter(([, value]) => value !== undefined);
  if (!entries.length) return false;
  const sets = entries.map(([column], i) => `${column} = $${i + 1}`);
  await db.execute(
    `UPDATE ${table} SET ${sets.join(", ")}, updated_at = datetime('now')
      WHERE id = $${entries.length + 1}`,
    [...entries.map(([, value]) => value), id],
  );
  return true;
}
