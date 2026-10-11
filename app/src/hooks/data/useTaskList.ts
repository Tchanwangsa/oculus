import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import {
  getAllTasks,
  getProjects,
  getUnfiledTasks,
  PROJECTS_UPDATED_EVENT,
  type DbProject,
  type DbTaskWithProject,
} from "@/lib/planning/projects";
import { useWindowEvent } from "@/hooks/backend/useEvents";

export type TaskScope = "all" | "unfiled";

export interface TaskList {
  tasks: DbTaskWithProject[];
  projects: DbProject[];
  /** The projects by id, for `boardOf`. An unfiled task simply misses. */
  projectById: Map<number, DbProject>;
  /** False until the first read lands, so "not read yet" is not "no tasks". */
  loaded: boolean;
  reload: () => void;
}

/**
 * Every task in `scope`, plus every project (archived too) so `boardOf` can
 * resolve a task's columns. Not `projectsStore`, which holds one open project.
 * Refreshes on `PROJECTS_UPDATED_EVENT`. `null` reads nothing, since a hook
 * cannot be called conditionally.
 */
export function useTaskList(scope: TaskScope | null): TaskList {
  const [tasks, setTasks] = useState<DbTaskWithProject[]>([]);
  const [projects, setProjects] = useState<DbProject[]>([]);
  const [loaded, setLoaded] = useState(false);

  // Reads can land out of order under a burst of writes; only the newest sets
  // state.
  const run = useRef(0);

  const load = useCallback(async () => {
    if (scope == null) return;
    const token = ++run.current;
    const [rows, list] = await Promise.all([
      scope === "unfiled" ? getUnfiledTasks() : getAllTasks(),
      getProjects({ status: "all" }),
    ]);
    if (run.current !== token) return;
    setTasks(rows);
    setProjects(list);
    setLoaded(true);
  }, [scope]);

  useEffect(() => {
    if (scope == null) {
      setLoaded(true);
      return;
    }
    void load().catch((e) => console.error("read tasks failed", e));
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

  return { tasks, projects, projectById, loaded, reload };
}
