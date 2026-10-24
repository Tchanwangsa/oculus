import { getDb } from "@/lib/db/connection";
import type {
  RevisionScope,
  RevisionNote,
  RevisionItem,
  RevisionSet,
  RevisionArtifact,
  RevisionReview,
  RevisionTopic,
} from "./types";
export function scopeQuery(scope: RevisionScope): {
  sql: string;
  params: unknown[];
} {
  return scope === "all"
    ? { sql: "", params: [] }
    : scope === "general"
      ? { sql: " WHERE subject_id IS NULL", params: [] }
      : { sql: " WHERE subject_id = $1", params: [scope] };
}
function unpack<T>(row: Record<string, unknown>, keys: string[]): T {
  const next = { ...row };
  for (const key of keys)
    next[key] =
      typeof row[key] === "string" ? JSON.parse(row[key] as string) : row[key];
  return next as T;
}
async function list<T>(
  table: string,
  scope: RevisionScope,
  order: string,
  json: string[] = [],
): Promise<T[]> {
  const db = await getDb();
  const q = scopeQuery(scope);
  const membership = {
    revision_notes: ["revision_note_topics", "note_id"],
    revision_items: ["revision_item_topics", "item_id"],
    revision_sets: ["revision_set_topics", "set_id"],
    revision_artifacts: ["revision_artifact_topics", "artifact_id"],
  }[table];
  const columns = membership
    ? `*, (SELECT json_group_array(topic_id) FROM ${membership[0]} WHERE ${membership[1]} = ${table}.id) AS topic_ids`
    : "*";
  const rows = await db.select<Record<string, unknown>[]>(
    `SELECT ${columns} FROM ${table}${q.sql} ORDER BY ${order}`,
    q.params,
  );
  return rows.map((row) =>
    unpack<T>(row, membership ? [...json, "topic_ids"] : json),
  );
}
export const getTopics = () =>
  list<RevisionTopic>("revision_topics", "all", "position, name, id");
export const getNotes = (scope: RevisionScope = "all") =>
  list<RevisionNote>("revision_notes", scope, "position, id", ["tags"]);
export const getItems = (scope: RevisionScope = "all") =>
  list<RevisionItem>("revision_items", scope, "created_at DESC, id DESC", [
    "options",
    "rubric",
    "topic",
  ]);
export const getSets = (scope: RevisionScope = "all") =>
  list<RevisionSet>("revision_sets", scope, "updated_at DESC, id DESC");
export const getArtifacts = (scope: RevisionScope = "all") =>
  list<RevisionArtifact>(
    "revision_artifacts",
    scope,
    "updated_at DESC, id DESC",
    ["state"],
  );
export async function getSetItems(setId: number): Promise<RevisionItem[]> {
  const db = await getDb();
  const rows = await db.select<Record<string, unknown>[]>(
    "SELECT i.*, (SELECT json_group_array(topic_id) FROM revision_item_topics WHERE item_id = i.id) AS topic_ids FROM revision_items i JOIN revision_set_items s ON i.id = s.item_id WHERE s.set_id = $1 ORDER BY s.position, i.id",
    [setId],
  );
  return rows.map((row) =>
    unpack<RevisionItem>(row, ["options", "rubric", "topic", "topic_ids"]),
  );
}
export async function getReviews(setId?: number): Promise<RevisionReview[]> {
  const db = await getDb();
  return db.select<RevisionReview[]>(
    `SELECT * FROM revision_reviews${setId == null ? "" : " WHERE set_id = $1"} ORDER BY at DESC, id DESC`,
    setId == null ? [] : [setId],
  );
}
