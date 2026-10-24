import { invoke } from "@tauri-apps/api/core";
import { validateItem } from "./validation";
import { getDb } from "@/lib/db/connection";
import type {
  NoteInput,
  ItemInput,
  ArtifactInput,
  RevisionNote,
  RevisionArtifact,
  RevisionSet,
  RevisionEntity,
} from "./types";
export const REVISION_UPDATED_EVENT = "oculus:revision-updated";
export function notifyRevisionUpdated() {
  window.dispatchEvent(new CustomEvent(REVISION_UPDATED_EVENT));
}
async function create(entity: RevisionEntity, input: object): Promise<number> {
  const id = await invoke<number>("revision_create_entity", { entity, input });
  notifyRevisionUpdated();
  return id;
}
export async function updateRevisionEntity(
  entity: RevisionEntity,
  id: number,
  patch: object,
) {
  await invoke("revision_update_entity", { entity, id, patch });
  notifyRevisionUpdated();
}
export const createNote = (input: NoteInput) =>
  create("note", { ...input, title: input.title.trim() || "Untitled" });
export const updateNote = (
  id: number,
  patch: Partial<
    Pick<
      RevisionNote,
      "title" | "body" | "subject_id" | "tags" | "position" | "topic_ids"
    >
  >,
) => updateRevisionEntity("note", id, patch);
export const createItem = (input: ItemInput) => {
  validateItem(input);
  return create("item", input);
};
export const updateItem = (id: number, patch: Partial<ItemInput>) =>
  updateRevisionEntity("item", id, patch);
export const createArtifact = (input: ArtifactInput) =>
  create("artifact", input);
export const updateArtifact = (
  id: number,
  patch: Partial<
    Pick<
      RevisionArtifact,
      "title" | "program" | "state" | "subject_id" | "topic_ids"
    >
  >,
) => updateRevisionEntity("artifact", id, patch);
export const updateSet = (
  id: number,
  patch: Partial<
    Pick<RevisionSet, "title" | "description" | "subject_id" | "topic_ids">
  >,
) => updateRevisionEntity("set", id, patch);
export async function createTopic(subjectId: number, name: string) {
  const id = await invoke<number>("revision_create_topic", { subjectId, name });
  notifyRevisionUpdated();
  return id;
}
export async function updateTopic(
  id: number,
  patch: { name?: string; description?: string; position?: number },
) {
  await invoke("revision_update_topic", { id, patch });
  notifyRevisionUpdated();
}
export async function deleteTopic(id: number) {
  await invoke("revision_delete_topic", { id });
  notifyRevisionUpdated();
}
async function remove(table: string, id: number) {
  const db = await getDb();
  await db.execute(`DELETE FROM ${table} WHERE id=$1`, [id]);
  notifyRevisionUpdated();
}
export const deleteNote = (id: number) => remove("revision_notes", id);
export const deleteItem = (id: number) => remove("revision_items", id);
export const deleteArtifact = (id: number) => remove("revision_artifacts", id);
export const deleteSet = (id: number) => remove("revision_sets", id);
export const createSet = (input: {
  title: string;
  subject_id?: number | null;
  description?: string;
  topic_ids?: number[];
}) => create("set", input);
export async function addSetItem(setId: number, itemId: number) {
  const db = await getDb();
  await db.execute(
    "INSERT OR IGNORE INTO revision_set_items (set_id,item_id,position) SELECT $1,$2,COALESCE(MAX(position),-1)+1 FROM revision_set_items WHERE set_id=$1",
    [setId, itemId],
  );
  notifyRevisionUpdated();
}
export async function removeSetItem(setId: number, itemId: number) {
  const db = await getDb();
  await db.execute(
    "DELETE FROM revision_set_items WHERE set_id=$1 AND item_id=$2",
    [setId, itemId],
  );
  notifyRevisionUpdated();
}
/** Places a note between two others; `patch` lands in the same write, so a
 *  move to another topic re-reads once. */
export async function moveNote(
  id: number,
  beforeId: number | null,
  afterId: number | null,
  patch: Partial<Pick<RevisionNote, "topic_ids">> = {},
) {
  const db = await getDb();
  const rows = await db.select<{ id: number; position: number }[]>(
    "SELECT id,position FROM revision_notes WHERE id IN ($1,$2)",
    [beforeId, afterId],
  );
  const before = rows.find((r) => r.id === beforeId)?.position;
  const after = rows.find((r) => r.id === afterId)?.position;
  await updateNote(id, {
    position:
      before == null
        ? (after ?? 1) - 1
        : after == null
          ? before + 1
          : (before + after) / 2,
    ...patch,
  });
}

export async function createQuiz(
  title: string,
  subject_id: number,
  items: number[],
  topicIds?: number[],
) {
  const id = await invoke<number>("revision_create_set", {
    title,
    subjectId: subject_id,
    items,
    topicIds,
  });
  notifyRevisionUpdated();
  return id;
}
