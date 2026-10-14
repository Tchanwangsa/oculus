import type { DbTaskWithProject } from "@/lib/planning/projects";
import { matchesDue } from "./filterRules";

/**
 * The Tasks nav column's views and tag, which live in the URL
 * (`/tasks?view=week&tag=3`) so ⌘-click, tab restore and history keep them.
 * Both compose with the page's search and filters.
 */

export const TASK_VIEWS = [
  { id: "all", label: "All tasks" },
  { id: "week", label: "Due this week" },
  { id: "undated", label: "No due date" },
  { id: "agent", label: "Added by the agent" },
] as const;

export type TaskViewId = (typeof TASK_VIEWS)[number]["id"];

export const DEFAULT_VIEW: TaskViewId = "all";

/** An unknown or missing `view` is the default one. */
export function parseView(raw: string | null): TaskViewId {
  return TASK_VIEWS.find((v) => v.id === raw)?.id ?? DEFAULT_VIEW;
}

/** A whole, positive `tag` id, else no tag. */
export function parseTagParam(raw: string | null): number | null {
  if (raw == null || !/^\d+$/.test(raw)) return null;
  const id = Number(raw);
  return Number.isSafeInteger(id) && id > 0 ? id : null;
}

/** `/tasks`, naming the view only off the default. */
export function tasksHref(view: TaskViewId, tag: number | null): string {
  const query = new URLSearchParams();
  if (view !== DEFAULT_VIEW) query.set("view", view);
  if (tag != null) query.set("tag", String(tag));
  const text = query.toString();
  return text ? `/tasks?${text}` : "/tasks";
}

export function viewLabel(view: TaskViewId): string {
  return TASK_VIEWS.find((v) => v.id === view)?.label ?? "All tasks";
}

/** `now` is passed in so one pass over the list reads one clock. */
export function matchesView(task: DbTaskWithProject, view: TaskViewId, now: number): boolean {
  switch (view) {
    case "all":
      return true;
    case "week":
      return matchesDue(task.due_at, "week", now);
    case "undated":
      return matchesDue(task.due_at, "undated", now);
    case "agent":
      return task.source === "agent";
  }
}

/** Every word of the query must appear in the title, the project's name or a
 *  tag's name, in any order; an empty query matches everything. */
export function matchesSearch(
  task: DbTaskWithProject,
  query: string,
  tagName: (id: number) => string | undefined,
): boolean {
  const words = query.toLowerCase().split(/\s+/).filter(Boolean);
  if (words.length === 0) return true;
  const haystack = [task.title, task.project_name ?? "", ...task.tag_ids.map((id) => tagName(id) ?? "")]
    .join("\n")
    .toLowerCase();
  return words.every((w) => haystack.includes(w));
}
