/**
 * Projects: a piece of work scoped to one subject or none, broken into tasks
 * and one level of subtask. Direct SQL over `getDb()`; the `oculus` CLI writes
 * the same rows through `app/src-tauri/src/db/projects/`, so a change to a
 * table's shape moves both writers together.
 *
 * A task with `project_id` NULL is *unfiled* — it belongs to no project; its
 * board is {@link boardOf}'s default.
 */
export { DEFAULT_COLUMNS, boardOf } from "./columns";
export type { ColumnKind, ProjectColumn } from "./columns";
export { normaliseTags, taskBodyEdit } from "./rows";
export type {
  DbOpenTask,
  DbProject,
  DbProjectTask,
  DbTaskWithProject,
} from "./rows";
export * from "./events";
export * from "./reads";
export * from "./projectWrites";
export * from "./taskWrites";
