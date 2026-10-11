import type { DbProject } from "./rows";

/** What a board column means (the name is the user's): `done` stamps
 *  `done_at`, `backlog` is the one the board can leave out. */
export type ColumnKind = "backlog" | "active" | "done";

export interface ProjectColumn {
  id: string;
  name: string;
  kind: ColumnKind;
}

/** The board a new project opens with; only `kind` is load-bearing. Frozen
 *  because it is shared — mutate a {@link freshColumns} copy instead. */
export const DEFAULT_COLUMNS: readonly ProjectColumn[] = Object.freeze([
  Object.freeze({ id: "backlog", name: "Backlog", kind: "backlog" }),
  Object.freeze({ id: "todo", name: "Todo", kind: "active" }),
  Object.freeze({ id: "doing", name: "In progress", kind: "active" }),
  Object.freeze({ id: "done", name: "Done", kind: "done" }),
] as ProjectColumn[]);

export function freshColumns(): ProjectColumn[] {
  return DEFAULT_COLUMNS.map((c) => ({ ...c }));
}

/**
 * The board a task's column is checked and drawn against: the project's own,
 * or the default for an unfiled task (whose `column_id` is still NOT NULL and
 * names a default column id). Mirrors `board_of` in `app/src-tauri/src/db/projects/columns.rs`.
 */
export function boardOf(project: DbProject | null | undefined): ProjectColumn[] {
  return project?.columns ?? freshColumns();
}
