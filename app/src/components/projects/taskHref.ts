import type { DbProjectTask } from "@/lib/projects";

/**
 * A task's own page, with its title along for the ride.
 *
 * The same trade `projectHref` makes one level up, and for the same reason:
 * `tabInfo` names a tab from the path alone and has no task list to look one
 * up in, so the title travels in `?n=`. A rename therefore leaves an old tab
 * titled with the old title until it is reopened — which is why the page
 * re-navigates to its own href after committing one (see `TaskPage`), so the
 * tab you are actually looking at re-titles itself.
 *
 * **Two shapes, because a task may belong to no project** (migration 37). A
 * filed task keeps its project in the path — the page reads the whole project,
 * whose columns are what a status *means*, and a task id alone would need a
 * lookup before the page could draw anything. An unfiled one has no project to
 * put there, so it lives under the universal Tasks page at `/tasks/:taskId`
 * and reads its board from `boardOf(null)`.
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
