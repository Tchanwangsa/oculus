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
export { MAX_TAG_NAME, normaliseTagName, normaliseTags, parseTagIds, taskBodyEdit } from "./rows";
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
export {
  TAG_COLORS,
  createTaskTag,
  deleteTaskTag,
  getTaskTags,
  isTagColor,
  setTaskTags,
  updateTaskTag,
} from "./tags";
export type { DbTaskTag, TagColor, UpdateTaskTagInput } from "./tags";
export * from "./taskFiles";
