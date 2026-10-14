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
  /** {@link UNIVERSAL_COLUMNS} ids to show — on the board, the columns that
   *  exist. Never empty: the control won't uncheck the last one. */
  status: readonly string[];
  /** Not `number | null`: `ProjectPicker`'s `null` already means Unfiled. */
  project: "any" | "unfiled" | number;
  /** `"personal"` is no subject at all; otherwise a subject code. */
  subject: "any" | "personal" | string;
  due: DueFilter;
  /** Read only while `due` is `"custom"`; the controls keep it
   *  {@link normaliseRange}d. */
  dueRange: DueRange;
  /** Registry tag ids, any of; empty is every task. */
  tags: readonly number[];
}

export const DEFAULT_FILTER: TaskFilter = {
  status: UNIVERSAL_COLUMNS.map((c) => c.id),
  project: "any",
  subject: "any",
  due: "any",
  dueRange: NO_DUE_RANGE,
  tags: [],
};

/** Whether the due section narrows: a Custom range with no ends is Any time. */
function dueNarrows(filter: TaskFilter): boolean {
  if (filter.due === "custom") return filter.dueRange.from != null || filter.dueRange.to != null;
  return filter.due !== "any";
}

/** How many of the Filter popover's sections (status, due, tags) narrow the
 *  list; project and subject have their own toolbar pills. */
export function popoverFilterCount(filter: TaskFilter): number {
  return (
    (filter.status.length !== UNIVERSAL_COLUMNS.length ? 1 : 0) +
    (dueNarrows(filter) ? 1 : 0) +
    (filter.tags.length > 0 ? 1 : 0)
  );
}

/** How many of the filter's sections narrow the list, project and subject
 *  included. */
export function activeFilterCount(filter: TaskFilter): number {
  return popoverFilterCount(filter) + (filter.project !== "any" ? 1 : 0) + (filter.subject !== "any" ? 1 : 0);
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
