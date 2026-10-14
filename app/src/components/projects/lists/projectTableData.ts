import type { DbProject, DbTaskWithProject } from "@/lib/planning/projects";
import { sqliteUtcToMs } from "@/lib/format/format";
import { matchesSearch } from "../tasks/taskViews";
import { sortTasks, type TaskSort } from "../tasks/taskSort";
import { taskTree, type TaskNode, type SubtaskProgress } from "../tasks/taskTree";

export interface ProjectBranch {
  project: DbProject;
  nodes: TaskNode[];
  /** Includes every task, even when search hides it. */
  progress: SubtaskProgress;
}

function compareProjects(a: DbProject, b: DbProject, sort: TaskSort): number {
  const dir = sort.dir === "asc" ? 1 : -1;
  let cmp = 0;
  if (sort.key === "due") {
    const am = sqliteUtcToMs(a.due_at), bm = sqliteUtcToMs(b.due_at);
    if (am == null || bm == null) return am == null ? (bm == null ? a.name.localeCompare(b.name) : 1) : -1;
    cmp = am - bm;
  } else if (sort.key === "updated" || sort.key === "created") {
    const field = sort.key === "updated" ? "updated_at" : "created_at";
    cmp = (sqliteUtcToMs(a[field]) ?? 0) - (sqliteUtcToMs(b[field]) ?? 0);
  } else cmp = a.name.localeCompare(b.name);
  return cmp * dir || a.name.localeCompare(b.name) || a.id - b.id;
}

/** Search preserves matching tasks' project and parent context; progress uses the full list. */
export function projectTableData(projects: DbProject[], tasks: DbTaskWithProject[], query: string,
  sort: TaskSort, tagName: (id: number) => string | undefined): ProjectBranch[] {
  const byProject = new Map<number, DbTaskWithProject[]>();
  for (const task of tasks) {
    if (task.project_id == null) continue;
    const rows = byProject.get(task.project_id) ?? [];
    rows.push(task);
    byProject.set(task.project_id, rows);
  }
  const projectById = new Map(projects.map(p => [p.id, p]));
  const words = query.toLowerCase().split(/\s+/).filter(Boolean);
  return [...projects].sort((a, b) => compareProjects(a, b, sort)).flatMap(project => {
    const all = byProject.get(project.id) ?? [];
    const haystack = [project.name, project.subject_code ?? "Personal", ...project.tags].join(" ").toLowerCase();
    const projectMatches = words.every(w => haystack.includes(w));
    const matches = new Set(all.filter(t => matchesSearch(t, query, tagName)).map(t => t.id));
    const parents = new Set(all.filter(t => matches.has(t.id) && t.parent_id != null).map(t => t.parent_id));
    const shown = projectMatches ? all : all.filter(t => matches.has(t.id) || parents.has(t.id) || (t.parent_id != null && matches.has(t.parent_id)));
    if (!projectMatches && shown.length === 0) return [];
    const total = all.length;
    const done = all.filter(t => t.done_at != null).length;
    return [{ project, nodes: taskTree(sortTasks(shown, sort, projectById)), progress: {done, total, pct: total ? Math.round(done / total * 100) : 0} }];
  });
}
