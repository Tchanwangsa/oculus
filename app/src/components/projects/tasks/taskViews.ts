import type { DbTaskWithProject } from "@/lib/planning/projects";

/**
 * The Tasks nav column's view, tag and subject scope live in the URL
 * (`/tasks?view=agent&tag=3`) so ⌘-click, tab restore and history keep them.
 * They compose with the page's search and filters.
 */

export const TASK_VIEWS = [
  { id: "all", label: "All tasks" },
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

export type TaskSubjectScope = number | "personal" | null;

export function parseSubjectParam(raw: string | null): TaskSubjectScope {
  return raw === "personal" ? "personal" : parseTagParam(raw);
}

/** `/tasks`, with optional view, tag and subject scope. */
export function tasksHref(view: TaskViewId, tag: number | null, subject: TaskSubjectScope = null): string {
  const query = new URLSearchParams();
  if (view !== DEFAULT_VIEW) query.set("view", view);
  if (tag != null) query.set("tag", String(tag));
  if (subject != null) query.set("subject", String(subject));
  const text = query.toString();
  return text ? `/tasks?${text}` : "/tasks";
}

export function viewLabel(view: TaskViewId): string {
  return TASK_VIEWS.find((v) => v.id === view)?.label ?? "All tasks";
}

export function matchesView(task: DbTaskWithProject, view: TaskViewId): boolean {
  switch (view) {
    case "all":
      return true;
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
