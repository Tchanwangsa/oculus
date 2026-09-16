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
 * The project id is in the path rather than only the task id because the page
 * reads the whole project — its columns are what a status means — and a task
 * id alone would need a lookup before the page could draw anything.
 */
export function taskHref(
  projectId: number,
  task: Pick<DbProjectTask, "id" | "title">,
): string {
  return `/projects/${projectId}/tasks/${task.id}?n=${encodeURIComponent(task.title)}`;
}
