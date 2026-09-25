import type { DbProject } from "@/lib/projects";

/** A project's route. `?n=` carries the name because `tabInfo` titles a tab
 *  from the path alone; a rename leaves old tabs on the old name. */
export function projectHref(project: Pick<DbProject, "id" | "name">): string {
  return `/projects/${project.id}?n=${encodeURIComponent(project.name)}`;
}
