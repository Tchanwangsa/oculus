import { freshColumns, type ProjectColumn } from "./columns";

/** A project row plus its subject's code (joined, never stored).
 *  `subject_id` NULL is a personal project. */
export interface DbProject {
  id: number;
  subject_id: number | null;
  subject_code: string | null;
  name: string;
  brief: string | null;
  /** 'active' | 'archived'. */
  status: string;
  starts_at: string | null;
  due_at: string | null;
  /** Parsed from the stored JSON. */
  columns: ProjectColumn[];
  /** Parsed from JSON; `[]` when untagged, never `null`. */
  tags: string[];
  /**
   * A `CalEvent.id` as `app/src/lib/planning/calendar/` mints it. Resolved live, not a
   * foreign key — a sync re-inserts Canvas rows — so a stale id draws nothing.
   */
  event_id: string | null;
  position: number;
  /** 'manual' | 'agent' — who created it. */
  source: string;
  created_at: string;
  updated_at: string;
}

export interface DbProjectTask {
  id: number;
  /** `null` on an unfiled task. */
  project_id: number | null;
  /** Non-null on a subtask. Subtasks are one level deep — see `createTask`. */
  parent_id: number | null;
  title: string;
  body: string | null;
  column_id: string;
  position: number;
  starts_at: string | null;
  due_at: string | null;
  estimate_minutes: number | null;
  /** Set while the task sits in a `kind: "done"` column; `moveTask` owns it. */
  done_at: string | null;
  source: string;
  created_at: string;
  updated_at: string;
}

/** A task with enough of its project to draw and open it. The project
 *  fields are all `null` on an unfiled task. */
export interface DbTaskWithProject extends DbProjectTask {
  project_name: string | null;
  project_subject_id: number | null;
  project_subject_code: string | null;
}

/** A dated, unfinished task — what the calendar's task layer reads. */
export type DbOpenTask = DbTaskWithProject;

/** The row as SQLite returns it, before `columns` and `tags` are parsed. */
export type ProjectRow = Omit<DbProject, "columns" | "tags"> & {
  columns: string;
  tags: string | null;
};

/** The stored JSON tags, or `[]`; non-strings are dropped. */
export function parseTags(raw: string | null): string[] {
  if (!raw) return [];
  try {
    const parsed: unknown = JSON.parse(raw);
    return Array.isArray(parsed) ? parsed.filter((t): t is string => typeof t === "string") : [];
  } catch {
    return [];
  }
}

export function toProject(row: ProjectRow): DbProject {
  let columns: ProjectColumn[];
  try {
    const parsed = JSON.parse(row.columns);
    columns = Array.isArray(parsed) && parsed.length ? parsed : freshColumns();
  } catch {
    // Fall back rather than throw: one bad row mustn't take the list down.
    columns = freshColumns();
  }
  return { ...row, columns, tags: parseTags(row.tags) };
}

/** Tags as stored: trimmed, deduplicated case-insensitively (first spelling
 *  wins) and capped — normalised on write so reads can compare strings. */
export function normaliseTags(tags: string[]): string[] {
  const seen = new Set<string>();
  const out: string[] = [];
  for (const raw of tags) {
    const tag = raw.trim().replace(/\s+/g, " ");
    if (!tag) continue;
    const key = tag.toLowerCase();
    if (seen.has(key)) continue;
    seen.add(key);
    out.push(tag);
    if (out.length >= MAX_TAGS) break;
  }
  return out;
}

const MAX_TAGS = 24;

/** The body write an edited description makes: trimmed, empty as `null`, and
 *  `undefined` when it matches what is stored (nothing to write). */
export function taskBodyEdit(text: string, stored: string | null): string | null | undefined {
  const body = text.trim();
  if (body === (stored ?? "")) return undefined;
  return body || null;
}
