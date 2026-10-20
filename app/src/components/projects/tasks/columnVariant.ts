import { boardOf, type DbProject, type ProjectColumn } from "@/lib/planning/projects";

/** What a column looks like: its glyph and its pill. Todo and In progress are
 *  both `active`, so the kind alone cannot tell them apart. */
export type ColumnVariant = "backlog" | "todo" | "doing" | "done";

/**
 * A column's variant. Kind first for backlog and done; among the active
 * columns a default id (`todo`, `doing`) names its own variant, otherwise the
 * last active column is in flight and the earlier ones are queued. Null when
 * the board has no such column.
 */
export function columnVariant(columns: readonly ProjectColumn[], columnId: string): ColumnVariant | null {
  const column = columns.find((c) => c.id === columnId);
  if (!column) return null;
  if (column.kind === "done") return "done";
  if (column.kind === "backlog") return "backlog";
  if (column.id === "todo" || column.id === "doing") return column.id;

  const active = columns.filter((c) => c.kind === "active");
  return active[active.length - 1].id === column.id ? "doing" : "todo";
}

/** {@link columnVariant} on a task's own board; `project` is null when unfiled. */
export function taskVariant(project: DbProject | null, columnId: string): ColumnVariant | null {
  return columnVariant(boardOf(project), columnId);
}
