import type { DbProject } from "@/lib/projects";

/**
 * A project's route, with its name along for the ride.
 *
 * The `?n=` is the tab strip's, not the router's: `tabInfo` names a tab from
 * the path alone and has no project list to look one up in, so the name
 * travels in the query the way a lecture's title does (`?t=`). A rename
 * therefore leaves an old tab titled with the old name until it is reopened,
 * which is the same trade the lecture route already makes.
 */
export function projectHref(project: Pick<DbProject, "id" | "name">): string {
  return `/projects/${project.id}?n=${encodeURIComponent(project.name)}`;
}
