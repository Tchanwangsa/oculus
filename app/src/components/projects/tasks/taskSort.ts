import { sqliteUtcToMs } from "@/lib/format/format";
import type { DbProject, DbTaskWithProject } from "@/lib/planning/projects";
import { UNIVERSAL_COLUMNS, projectLabel, universalColumnOf } from "./universalTasks";

/** What the Display menu and the table's column headers sort by. */
export const SORT_KEYS = ["updated", "due", "title", "status", "project", "created"] as const;

export type SortKey = (typeof SORT_KEYS)[number];
export type SortDir = "asc" | "desc";

export interface TaskSort {
  key: SortKey;
  dir: SortDir;
}

export const SORT_LABELS: Record<SortKey, string> = {
  updated: "Updated",
  due: "Due",
  title: "Title",
  status: "Status",
  project: "Project",
  created: "Created",
};

export function isSortKey(value: unknown): value is SortKey {
  return (SORT_KEYS as readonly unknown[]).includes(value);
}

/** Parsed, not string-compared: `due_at` is ISO from the UI or SQLite's
 *  "YYYY-MM-DD HH:MM:SS" from the CLI, which don't sort together. Undated last. */
export function dueCmp(a: DbTaskWithProject, b: DbTaskWithProject): number {
  const am = sqliteUtcToMs(a.due_at);
  const bm = sqliteUtcToMs(b.due_at);
  if (am == null || bm == null) return am == null ? (bm == null ? 0 : 1) : -1;
  return am - bm;
}

/** `UNIVERSAL_ORDER` from `app/src/lib/planning/projects/reads.ts`, mirrored by
 *  hand — keep the two in step. The tiebreak under every sort; Due ascending
 *  reproduces the arrival order. */
export function defaultCmp(a: DbTaskWithProject, b: DbTaskWithProject): number {
  const due = dueCmp(a, b);
  if (due !== 0) return due;
  // Unfiled first.
  if ((a.project_id == null) !== (b.project_id == null)) return a.project_id == null ? -1 : 1;
  if (a.project_id != null && b.project_id != null && a.project_id !== b.project_id) {
    return a.project_id - b.project_id;
  }
  return a.position - b.position || a.id - b.id;
}

function stampCmp(a: string, b: string): number {
  return (sqliteUtcToMs(a) ?? 0) - (sqliteUtcToMs(b) ?? 0);
}

/** A copy of `tasks` in `sort`'s order. Undated stays last in both directions;
 *  every tie falls to {@link defaultCmp}. */
export function sortTasks(
  tasks: readonly DbTaskWithProject[],
  sort: TaskSort,
  projectById: Map<number, DbProject>,
): DbTaskWithProject[] {
  const keyCmp = (a: DbTaskWithProject, b: DbTaskWithProject): number => {
    switch (sort.key) {
      case "title":
        return a.title.localeCompare(b.title);
      // By the cell's text, so "No project" sorts among the names.
      case "project":
        return projectLabel(a).localeCompare(projectLabel(b));
      // By board column order — not by name, and not by kind (Todo and In
      // progress share a kind but are separate columns).
      case "status":
        return (
          UNIVERSAL_COLUMNS.indexOf(universalColumnOf(a, projectById)) -
          UNIVERSAL_COLUMNS.indexOf(universalColumnOf(b, projectById))
        );
      case "due":
        return dueCmp(a, b);
      case "updated":
        return stampCmp(a.updated_at, b.updated_at);
      case "created":
        return stampCmp(a.created_at, b.created_at);
    }
  };
  return [...tasks].sort((a, b) => {
    if (sort.key === "due") {
      const am = a.due_at == null;
      const bm = b.due_at == null;
      if (am !== bm) return am ? 1 : -1;
    }
    const k = keyCmp(a, b);
    if (k !== 0) return sort.dir === "asc" ? k : -k;
    return defaultCmp(a, b);
  });
}
