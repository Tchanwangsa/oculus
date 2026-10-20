import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import {
  getAllTasks,
  getProjects,
  getUnfiledTasks,
  getTasks,
  PROJECTS_UPDATED_EVENT,
  type DbProject,
  type DbTaskWithProject,
} from "@/lib/planning/projects";
import { useWindowEvent } from "@/hooks/backend/useEvents";

export type TaskScope = "all" | "unfiled" | number;

export interface TaskList {
  tasks: DbTaskWithProject[];
  projects: DbProject[];
  /** The projects by id, for `boardOf`. An unfiled task simply misses. */
  projectById: Map<number, DbProject>;
  /** False until this scope's first read lands. */
  loaded: boolean;
  reload: () => void;
}

/**
 * Every task in `scope` (a project id, all tasks or unfiled), plus every
 * project (archived too) so `boardOf` can
 * resolve a task's columns. Not `projectsStore`, which holds one open project.
 * Refreshes on `PROJECTS_UPDATED_EVENT`. `null` reads nothing, since a hook
 * cannot be called conditionally.
 */
export function useTaskList(scope: TaskScope | null): TaskList {
  const [tasks, setTasks] = useState<DbTaskWithProject[]>([]);
  const [projects, setProjects] = useState<DbProject[]>([]);
  const [loadedScope, setLoadedScope] = useState<TaskScope | null>(null);

  // Reads can land out of order under a burst of writes; only the newest sets
  // state.
  const run = useRef(0);

  const load = useCallback(async () => {
    if (scope == null) return;
    const token = ++run.current;
    const [rows, list] = await Promise.all([
      typeof scope === "number" ? getTasks(scope) : scope === "unfiled" ? getUnfiledTasks() : getAllTasks(),
      getProjects({ status: "all" }),
    ]);
    if (run.current !== token) return;
    const projectById = new Map(list.map((p) => [p.id, p]));
    setTasks(rows.map((task) => {
      const project = task.project_id != null ? projectById.get(task.project_id) : null;
      return { ...task, project_name: project?.name ?? null,
        project_subject_id: project?.subject_id ?? null, project_subject_code: project?.subject_code ?? null };
    }));
    setProjects(list);
    setLoadedScope(scope);
  }, [scope]);

  useEffect(() => {
    if (scope == null) return;
    void load().catch((e) => console.error("read tasks failed", e));
    return () => { run.current++; };
  }, [load, scope]);
  useWindowEvent(PROJECTS_UPDATED_EVENT, () => {
    if (scope != null) void load().catch((e) => console.error("read tasks failed", e));
  });

  const projectById = useMemo(
    () => new Map(projects.map((p) => [p.id, p])),
    [projects],
  );

  const reload = useCallback(() => {
    void load().catch((e) => console.error("read tasks failed", e));
  }, [load]);

  return { tasks: loadedScope === scope ? tasks : [], projects, projectById, loaded: scope == null || loadedScope === scope, reload };
}
