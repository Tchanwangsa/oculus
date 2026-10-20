import { sqliteUtcToMs, startOfDay } from "@/lib/format/format";
import { addDays, startOfWeek } from "@/lib/planning/calendar/dates";
import type { DbProject, DbTaskWithProject, ProjectColumn } from "@/lib/planning/projects";
import { UNIVERSAL_COLUMNS, universalColumnOf } from "./universalTasks";

/**
 * The universal Tasks view's filters, as plain data. The page holds the state
 * and `TaskFilters.tsx` draws the controls; filtering is {@link matchesFilter}
 * over the one `getAllTasks` list, not a query per filter.
 */

export type DueFilter = "any" | "overdue" | "today" | "week" | "undated" | "custom";

/** The Custom due range: local-midnight epoch ms of the first and last day
 *  wanted, either end open. The last day counts whole. */
export interface DueRange {
  from: number | null;
  to: number | null;
}

export const NO_DUE_RANGE: DueRange = { from: null, to: null };

export interface TaskFilter {
  /** Selected {@link UNIVERSAL_COLUMNS} ids; empty means Any and shows all
   *  columns. An explicit Select all keeps the chips selected until reset. */
  status: readonly string[];
  /** Not `number | null`: `ProjectPicker`'s `null` already means Unfiled. */
  project: "any" | "unfiled" | number;
  /** `"personal"` is no subject at all; otherwise a subject code. */
  subject: "any" | "personal" | string;
  /** Include tasks belonging to archived projects. */
  showArchived: boolean;
  due: DueFilter;
  /** Read only while `due` is `"custom"`; the controls keep it
   *  {@link normaliseRange}d. */
  dueRange: DueRange;
  /** Registry tag ids, any of; empty is every task. */
  tags: readonly number[];
}

export const DEFAULT_FILTER: TaskFilter = {
  status: [],
  project: "any",
  subject: "any",
  showArchived: false,
  due: "any",
  dueRange: NO_DUE_RANGE,
  tags: [],
};

export function isDefaultFilter(filter: TaskFilter): boolean {
  return filter.status.length === 0 &&
    filter.project === "any" && filter.subject === "any" && filter.due === "any" &&
    filter.dueRange.from == null && filter.dueRange.to == null &&
    filter.tags.length === 0 && !filter.showArchived;
}

/** Empty means Any: picking one starts a selection, clearing the last
 * restores Any. Explicit Select all can be narrowed by unchecking a chip. */
export function toggleStatusFilter(status: readonly string[], id: string, columns: readonly ProjectColumn[] = UNIVERSAL_COLUMNS): readonly string[] {
  const next = status.includes(id) ? status.filter((value) => value !== id) : [...status, id];
  return next.length ? columns.filter((c) => next.includes(c.id)).map((c) => c.id) : DEFAULT_FILTER.status;
}

/** Choosing Custom counts as a changed filter even before either end is set. */
function dueChanged(filter: TaskFilter): boolean {
  return filter.due !== "any";
}

/** How many of the Filter popover's sections (status, due, tags, archived visibility) differ from
 *  their defaults; project and subject have their own toolbar pills. */
export function popoverFilterCount(filter: TaskFilter): number {
  return (
    (filter.status.length > 0 ? 1 : 0) +
    (dueChanged(filter) ? 1 : 0) +
    (filter.tags.length > 0 ? 1 : 0) +
    (filter.showArchived ? 1 : 0)
  );
}

/** How many of the filter's sections differ from default, project and subject
 *  included. */
export function activeFilterCount(filter: TaskFilter): number {
  return popoverFilterCount(filter) + (filter.project !== "any" ? 1 : 0) + (filter.subject !== "any" ? 1 : 0);
}

export function isFiltered(filter: TaskFilter): boolean {
  return activeFilterCount(filter) > 0;
}

export function filteredColumns(filter: TaskFilter, columns: readonly ProjectColumn[] = UNIVERSAL_COLUMNS): readonly ProjectColumn[] {
  if (filter.status.length === 0) return columns;
  return columns.filter((c) => filter.status.includes(c.id));
}

/** `now` is passed in so one pass over the list reads one clock. */
export function matchesFilter(
  task: DbTaskWithProject,
  filter: TaskFilter,
  projectById: Map<number, DbProject>,
  now: number,
  project?: DbProject,
): boolean {
  if (!project && !filter.showArchived && task.project_id != null && projectById.get(task.project_id)?.status === "archived") return false;
  if (filter.status.length > 0 && !filter.status.includes(project ? task.column_id : universalColumnOf(task, projectById).id)) return false;

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

  return matchesDue(task.due_at, filter.due, now, filter.dueRange);
}

/** A range whose ends are the wrong way round is swapped, so picking a
 *  From after the To (or the reverse) keeps both days. Ends snap to local
 *  midnight. */
export function normaliseRange({ from, to }: DueRange): DueRange {
  const day = (ms: number | null) => (ms == null ? null : startOfDay(new Date(ms)).getTime());
  const a = day(from);
  const b = day(to);
  return a != null && b != null && a > b ? { from: b, to: a } : { from: a, to: b };
}

/** Parsed, not string-compared: `due_at` may be ISO or SQLite's format (see
 *  `dueCmp` in `taskSort.ts`). "This week" is the calendar's week
 *  (`startOfWeek`), not a rolling seven days. Custom is the whole days
 *  `range` names, local time, either end open; with neither end it is Any
 *  time, and any range excludes an undated task. */
export function matchesDue(
  dueAt: string | null,
  due: DueFilter,
  now: number,
  range: DueRange = NO_DUE_RANGE,
): boolean {
  if (due === "any") return true;
  const ms = sqliteUtcToMs(dueAt);
  if (due === "custom") {
    const { from, to } = normaliseRange(range);
    if (from == null && to == null) return true;
    if (ms == null) return false;
    return (from == null || ms >= from) && (to == null || ms < addDays(new Date(to), 1).getTime());
  }
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
