import type { DbProjectTask } from "@/lib/projects";

/**
 * A task's page, title in `?n=` for the tab name (see {@link projectHref}).
 * A filed task keeps its project in the path; an unfiled one is `/tasks/:id`.
 */
export function taskHref(
  projectId: number | null,
  task: Pick<DbProjectTask, "id" | "title">,
): string {
  const query = `?n=${encodeURIComponent(task.title)}`;
  return projectId == null
    ? `/tasks/${task.id}${query}`
    : `/projects/${projectId}/tasks/${task.id}${query}`;
}
