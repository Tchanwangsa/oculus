import { getDb } from "@/lib/db";
import { notifyProjectsUpdated } from "./events";
import { normaliseTagName } from "./rows";
import { touchTask } from "./touch";

/** The only colours a tag takes — the Rust writer in
 *  `app/src-tauri/src/db/projects/` validates the same set, and
 *  `app/src/components/projects/tags/tagColors.ts` styles each. */
export const TAG_COLORS = [
  "slate",
  "indigo",
  "green",
  "amber",
  "rose",
  "purple",
  "teal",
  "orange",
] as const;

export type TagColor = (typeof TAG_COLORS)[number];

export function isTagColor(value: unknown): value is TagColor {
  return (TAG_COLORS as readonly unknown[]).includes(value);
}

/** One entry of the global tag registry. Tasks link to it by id. */
export interface DbTaskTag {
  id: number;
  name: string;
  color: TagColor;
  position: number;
}

const TAG_COLUMNS = `id, name, color, position`;

/** An unknown stored colour draws as slate rather than as nothing. */
function toTag(row: Omit<DbTaskTag, "color"> & { color: string }): DbTaskTag {
  return { ...row, color: isTagColor(row.color) ? row.color : "slate" };
}

function requireColor(color: string): TagColor {
  if (!isTagColor(color)) {
    throw new Error(`unknown tag colour "${color}" (has: ${TAG_COLORS.join(", ")})`);
  }
  return color;
}

/** Every tag, in registry order. */
export async function getTaskTags(): Promise<DbTaskTag[]> {
  const db = await getDb();
  const rows = await db.select<(Omit<DbTaskTag, "color"> & { color: string })[]>(
    `SELECT ${TAG_COLUMNS} FROM task_tags ORDER BY position ASC, id ASC`,
  );
  return rows.map(toTag);
}

async function tagByName(name: string): Promise<DbTaskTag | null> {
  const db = await getDb();
  const rows = await db.select<(Omit<DbTaskTag, "color"> & { color: string })[]>(
    `SELECT ${TAG_COLUMNS} FROM task_tags WHERE name = $1 COLLATE NOCASE`,
    [name],
  );
  return rows.length ? toTag(rows[0]) : null;
}

/**
 * Add a tag to the registry, last in order. A name that matches an existing
 * tag (case-insensitively) returns that tag rather than failing, so a picker's
 * "Create" never errors on a tag typed twice.
 */
export async function createTaskTag(
  name: string,
  color: TagColor = "slate",
): Promise<DbTaskTag> {
  const clean = normaliseTagName(name);
  requireColor(color);
  const existing = await tagByName(clean);
  if (existing) return existing;

  const db = await getDb();
  try {
    await db.execute(
      `INSERT INTO task_tags (name, color, position)
       VALUES ($1, $2, (SELECT COALESCE(MAX(position), 0) + 1 FROM task_tags))`,
      [clean, color],
    );
  } catch (e) {
    // The unique index lost a race with another writer: theirs wins.
    const raced = await tagByName(clean);
    if (raced) return raced;
    throw e;
  }
  const created = await tagByName(clean);
  if (!created) throw new Error("tag insert returned no row");
  notifyProjectsUpdated();
  return created;
}

export interface UpdateTaskTagInput {
  name?: string;
  color?: TagColor;
}

/** Rename or recolour a tag. A name another tag already has (ignoring case)
 *  is refused; changing only the case of its own name is fine. */
export async function updateTaskTag(id: number, patch: UpdateTaskTagInput): Promise<void> {
  const db = await getDb();
  const sets: string[] = [];
  const values: unknown[] = [];
  if (patch.name !== undefined) {
    const name = normaliseTagName(patch.name);
    const clash = await tagByName(name);
    if (clash && clash.id !== id) throw new Error(`a tag named "${clash.name}" already exists`);
    values.push(name);
    sets.push(`name = $${values.length}`);
  }
  if (patch.color !== undefined) {
    values.push(requireColor(patch.color));
    sets.push(`color = $${values.length}`);
  }
  if (!sets.length) return;
  values.push(id);
  await db.execute(`UPDATE task_tags SET ${sets.join(", ")} WHERE id = $${values.length}`, values);
  notifyProjectsUpdated();
}

/** Delete a tag; its links to tasks go with it by cascade. */
export async function deleteTaskTag(id: number): Promise<void> {
  const db = await getDb();
  await db.execute(`DELETE FROM task_tags WHERE id = $1`, [id]);
  notifyProjectsUpdated();
}

/**
 * Make `tagIds` the task's tags. New links go in first and the rest come out
 * after, so a failure midway never leaves a task with fewer tags than it had.
 * Ids with no registry row are dropped. Writes no timestamp and fires no
 * event — {@link setTaskTags} and the task writers do that.
 */
export async function replaceTaskTagLinks(taskId: number, tagIds: number[]): Promise<void> {
  const db = await getDb();
  const wanted = JSON.stringify([...new Set(tagIds.filter(Number.isInteger))]);
  await db.execute(
    `INSERT OR IGNORE INTO task_tag_links (task_id, tag_id)
     SELECT $1, g.id FROM task_tags g
      WHERE g.id IN (SELECT value FROM json_each($2))`,
    [taskId, wanted],
  );
  await db.execute(
    `DELETE FROM task_tag_links
      WHERE task_id = $1 AND tag_id NOT IN (SELECT value FROM json_each($2))`,
    [taskId, wanted],
  );
}

/** Replace a task's tags (`[]` clears them). */
export async function setTaskTags(taskId: number, tagIds: number[]): Promise<void> {
  await replaceTaskTagLinks(taskId, tagIds);
  await touchTask(taskId);
  notifyProjectsUpdated();
}
