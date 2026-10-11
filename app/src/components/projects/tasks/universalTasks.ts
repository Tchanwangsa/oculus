import {
  boardOf,
  DEFAULT_COLUMNS,
  type ColumnKind,
  type DbProject,
  type DbTaskWithProject,
  type ProjectColumn,
} from "@/lib/planning/projects";
import { columnOf } from "./taskTree";

/**
 * Shaping for task views that span every project.
 *
 * A universal column has no manual order: `position` is only comparable within
 * one project's column, so mixed columns are sorted (`UNIVERSAL_ORDER`: due
 * date, project, position) and a drag within one column is a no-op. A drag
 * across columns changes the column on the task's own board
 * ({@link columnForUniversal}).
 */

/** The default board every project starts with; matched id first, kind as
 *  fallback ({@link universalColumnOf}). An alias so the two can't drift. */
export const UNIVERSAL_COLUMNS: readonly ProjectColumn[] = DEFAULT_COLUMNS;

/** Where a kind goes when an id cannot be matched. */
const HOME_OF_KIND: Record<ColumnKind, string> = {
  backlog: "backlog",
  active: "doing",
  done: "done",
};

/** The task's project, or `null` when unfiled (the default board). */
export function projectOf(
  task: DbTaskWithProject,
  projectById: Map<number, DbProject>,
): DbProject | null {
  return task.project_id != null ? projectById.get(task.project_id) ?? null : null;
}

/**
 * Which of {@link UNIVERSAL_COLUMNS} a task is drawn in: same id first (keeps
 * Todo and In progress apart — both are `active`), then the kind's home. A
 * column its board no longer has falls to Backlog rather than vanishing.
 */
export function universalColumnOf(
  task: DbTaskWithProject,
  projectById: Map<number, DbProject>,
): ProjectColumn {
  const backlog = UNIVERSAL_COLUMNS[0];
  const own = columnOf(projectOf(task, projectById), task.column_id);
  if (!own) return backlog;
  return (
    UNIVERSAL_COLUMNS.find((c) => c.id === own.id) ??
    UNIVERSAL_COLUMNS.find((c) => c.id === HOME_OF_KIND[own.kind]) ??
    backlog
  );
}

/**
 * The column on this task's own board that a drop onto `universalId` means:
 * the same id, else the first column of that kind, else `null` (refuse).
 */
export function columnForUniversal(
  task: DbTaskWithProject,
  projectById: Map<number, DbProject>,
  universalId: string,
): ProjectColumn | null {
  const board = boardOf(projectOf(task, projectById));
  const exact = board.find((c) => c.id === universalId);
  if (exact) return exact;
  const kind = UNIVERSAL_COLUMNS.find((c) => c.id === universalId)?.kind;
  if (!kind) return null;
  return board.find((c) => c.kind === kind) ?? null;
}

/**
 * `moveTask`'s `beforeId` for appending to a column: the last task of the
 * task's **own project** there — the only positions its own is comparable with.
 */
export function appendNeighbour(
  tasks: DbTaskWithProject[],
  task: DbTaskWithProject,
  columnId: string,
): number | null {
  let tail: DbTaskWithProject | null = null;
  for (const t of tasks) {
    if (t.id === task.id) continue;
    if (t.project_id !== task.project_id) continue;
    if (t.column_id !== columnId) continue;
    if (!tail || t.position > tail.position) tail = t;
  }
  return tail?.id ?? null;
}

export function projectLabel(task: DbTaskWithProject): string {
  return task.project_name ?? "Unfiled";
}
