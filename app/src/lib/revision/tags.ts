import { getDb } from "@/lib/db/connection";
import { isTagColor, type DbTaskTag } from "@/lib/planning/projects/tags";

/** A cheat sheet tag from `revision_tags` (migration 52). Same shape and
 *  palette as a task tag, so the task chips and picker draw it. Entries store
 *  names; a name the registry lacks is added by a trigger when it is saved. */
export type CheatTag = DbTaskTag;

/** Every cheat sheet tag, in registry order. */
export async function getCheatTags(): Promise<CheatTag[]> {
  const db = await getDb();
  const rows = await db.select<(Omit<CheatTag, "color"> & { color: string })[]>(
    "SELECT id, name, color, position FROM revision_tags ORDER BY position ASC, id ASC",
  );
  return rows.map((row) => ({ ...row, color: isTagColor(row.color) ? row.color : "slate" }));
}
