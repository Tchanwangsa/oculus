import { sqliteUtcToMs, startOfDay } from "@/lib/format/format";
import { addDays, startOfWeek } from "@/lib/planning/calendar/dates";
import type { DbProject, DbTaskWithProject, ProjectColumn } from "@/lib/planning/projects";
import { UNIVERSAL_COLUMNS, universalColumnOf } from "./universalTasks";

/**
 * The universal Tasks view's filters, as plain data. The page holds the state
 * and `TaskFilters.tsx` draws the controls; filtering is {@link matchesFilter}
 * over the one `getAllTasks` list, not a query per filter.
 */

export type DueFilter = "any" | "overdue" | "today" | "week" | "undated";

export interface TaskFilter {
  /** {@link UNIVERSAL_COLUMNS} ids to show — on the board, the columns that
   *  exist. Never empty: the control won't uncheck the last one. */
  status: readonly string[];
  /** Not `number | null`: `ProjectPicker`'s `null` already means Unfiled. */
  project: "any" | "unfiled" | number;
  /** `"personal"` is no subject at all; otherwise a subject code. */
  subject: "any" | "personal" | string;
  due: DueFilter;
  /** Registry tag ids, any of; empty is every task. */
  tags: readonly number[];
}

export const DEFAULT_FILTER: TaskFilter = {
  status: UNIVERSAL_COLUMNS.map((c) => c.id),
  project: "any",
  subject: "any",
  due: "any",
  tags: [],
};

/** How many of the filter's sections narrow the list. */
export function activeFilterCount(filter: TaskFilter): number {
  return (
    (filter.status.length !== UNIVERSAL_COLUMNS.length ? 1 : 0) +
    (filter.project !== "any" ? 1 : 0) +
    (filter.subject !== "any" ? 1 : 0) +
    (filter.due !== "any" ? 1 : 0) +
    (filter.tags.length > 0 ? 1 : 0)
  );
}

export function isFiltered(filter: TaskFilter): boolean {
  return activeFilterCount(filter) > 0;
}

export function filteredColumns(filter: TaskFilter): readonly ProjectColumn[] {
  return UNIVERSAL_COLUMNS.filter((c) => filter.status.includes(c.id));
}

/** `now` is passed in so one pass over the list reads one clock. */
export function matchesFilter(
  task: DbTaskWithProject,
  filter: TaskFilter,
  projectById: Map<number, DbProject>,
  now: number,
): boolean {
  if (!filter.status.includes(universalColumnOf(task, projectById).id)) return false;

  if (filter.project === "unfiled") {
    if (task.project_id != null) return false;
  } else if (filter.project !== "any") {
    if (task.project_id !== filter.project) return false;
  }

  if (filter.subject === "personal") {
    if (task.project_subject_code != null) return false;
  } else if (filter.subject !== "any") {
    if (task.project_subject_code !== filter.subject) return false;
  }

  if (filter.tags.length > 0 && !task.tag_ids.some((id) => filter.tags.includes(id))) {
    return false;
  }

  return matchesDue(task.due_at, filter.due, now);
}

/** Parsed, not string-compared: `due_at` may be ISO or SQLite's format (see
 *  `dueCmp` in `taskSort.ts`). "This week" is the calendar's week
 *  (`startOfWeek`), not a rolling seven days. */
export function matchesDue(dueAt: string | null, due: DueFilter, now: number): boolean {
  if (due === "any") return true;
  const ms = sqliteUtcToMs(dueAt);
  if (due === "undated") return ms == null;
  if (ms == null) return false;
  if (due === "overdue") return ms < now;
  const today = new Date(now);
  if (due === "today") {
    const from = startOfDay(today).getTime();
    return ms >= from && ms < addDays(today, 1).getTime();
  }
  const from = startOfWeek(today).getTime();
  return ms >= from && ms < addDays(startOfWeek(today), 7).getTime();
}
