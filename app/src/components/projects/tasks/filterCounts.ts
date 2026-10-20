import type { DbProject, DbTaskWithProject } from "@/lib/planning/projects";
import { matchesFilter, type TaskFilter } from "./filterRules";

/** Counts each filter's alternatives against the other controls. Choosing a
 * subject clears the project; choosing an archived project includes archives. */
export function taskFilterCounts(
  tasks: readonly DbTaskWithProject[],
  filter: TaskFilter,
  projectById: Map<number, DbProject>,
  now: number,
) {
  const projects = new Map<number | null, number>();
  const subjects = new Map<string | null, number>();
  const projectFilter = { ...filter, project: "any" as const, showArchived: true };
  const subjectFilter = { ...filter, project: "any" as const, subject: "any" };
  let anyProject = 0;
  let anySubject = 0;
  for (const task of tasks) {
    if (matchesFilter(task, projectFilter, projectById, now)) {
      projects.set(task.project_id, (projects.get(task.project_id) ?? 0) + 1);
      if (filter.showArchived || task.project_id == null || projectById.get(task.project_id)?.status !== "archived") anyProject++;
    }
    if (matchesFilter(task, subjectFilter, projectById, now)) {
      subjects.set(task.project_subject_code, (subjects.get(task.project_subject_code) ?? 0) + 1);
      anySubject++;
    }
  }
  return { projects, subjects, anyProject, anySubject };
}

export type TaskFilterCounts = ReturnType<typeof taskFilterCounts>;

export function pickSubjectFilter(filter: TaskFilter, subject: TaskFilter["subject"]): TaskFilter {
  return { ...filter, subject: subject === filter.subject ? "any" : subject, project: "any" };
}
