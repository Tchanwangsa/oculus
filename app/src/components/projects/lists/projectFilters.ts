import type { DbTaskTag } from "@/lib/planning/projects";
import { matchesDue, type TaskFilter } from "../tasks/filterRules";
import type { TaskFilterCounts } from "../tasks/filterCounts";
import { universalColumnOf } from "../tasks/universalTasks";
import type { DbProject } from "@/lib/planning/projects";
import type { ProjectBranch } from "./projectTableData";

/** Project due/tags/subject apply to the project; Status matches any task on its board. */
export function matchesProjectFilter(branch: ProjectBranch, filter: TaskFilter,
  tags: Map<number, DbTaskTag>, projectById: Map<number, DbProject>, now: number): boolean {
  const { project, nodes } = branch;
  if (!filter.showArchived && project.status === "archived") return false;
  if (filter.subject === "personal" ? project.subject_code != null
    : filter.subject !== "any" && project.subject_code !== filter.subject) return false;
  if (filter.tags.length && !filter.tags.some(id => {
    const name = tags.get(id)?.name.toLowerCase();
    return name != null && project.tags.some(t => t.toLowerCase() === name);
  })) return false;
  if (filter.status.length) {
    const tasks = nodes.flatMap(n => [n.task, ...n.children]);
    if (!tasks.length ? !filter.status.includes("backlog")
      : !tasks.some(t => filter.status.includes(universalColumnOf(t, projectById).id))) return false;
  }
  return matchesDue(project.due_at, filter.due, now, filter.dueRange);
}

export function projectSubjectCounts(branches: ProjectBranch[], filter: TaskFilter,
  tags: Map<number, DbTaskTag>, projectById: Map<number, DbProject>, now: number): TaskFilterCounts {
  const subjects = new Map<string | null, number>();
  let anySubject = 0;
  for (const branch of branches) {
    if (!matchesProjectFilter(branch, { ...filter, subject: "any" }, tags, projectById, now)) continue;
    const code = branch.project.subject_code;
    subjects.set(code, (subjects.get(code) ?? 0) + 1);
    anySubject++;
  }
  return { projects: new Map(), subjects, anyProject: 0, anySubject };
}
