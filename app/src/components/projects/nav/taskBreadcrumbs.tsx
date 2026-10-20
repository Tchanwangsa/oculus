import { ListChecks } from "@phosphor-icons/react";
import { ProjectIcon } from "../icons/ProjectIcon";
import type { BreadcrumbItem } from "@/components/ui/Breadcrumb";
import type { DbProject, DbProjectTask } from "@/lib/planning/projects";
import { projectParents } from "./projectBreadcrumbs";
import { taskVariant } from "../tasks/columnVariant";
import { TaskGlyph } from "../tasks/TaskMarks";
import { projectHref } from "./projectHref";
import { taskHref } from "./taskHref";

/** The final two items identify the immediate parent and current task. */
export function taskBreadcrumbs(task: DbProjectTask, project: DbProject | null, parent: DbProjectTask | null): BreadcrumbItem[] {
  const items: BreadcrumbItem[] = project
    ? [...projectParents(project), { label: project.name, href: projectHref(project), icon: <ProjectIcon project={project} size={14} /> }]
    : [{ label: "Tasks", href: "/tasks", icon: <ListChecks size={12} /> }];
  if (parent) items.push({ label: parent.title, href: taskHref(task.project_id, parent),
    icon: <TaskGlyph variant={taskVariant(project, parent.column_id)} size={14} /> });
  items.push({ label: task.title, icon: <TaskGlyph variant={taskVariant(project, task.column_id)} size={14} /> });
  return items;
}
