import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import {
  getAllTasks,
  getProjects,
  getUnfiledTasks,
  PROJECTS_UPDATED_EVENT,
  type DbProject,
  type DbTaskWithProject,
} from "@/lib/projects";

/**
 * Every task there is, with the projects needed to draw them — what the
 * universal Tasks page reads, and what an unfiled task's own page reads.
 *
 * **Deliberately not `projectsStore`.** That store holds *one open project*
 * and that project's tasks (`activeId` + `tasks`), which is the right shape
 * for a board and exactly the wrong shape here: a view that spans every
 * project has no `activeId` to be, and a task filed nowhere would have to
 * pretend to belong to whatever was last opened. So this is a plain hook over
 * the same lib reads, and it refreshes by the one door everything else does —
 * `PROJECTS_UPDATED_EVENT` on `window`, which a click in the UI and a write the
 * chat agent made through `oculus task` both arrive by (`docs/projects.md`).
 *
 * The projects come along because a task's *board* does not: `DbTaskWithProject`
 * carries the project's name and subject, not its `columns`, and a column id
 * only means something against the board it was checked against. `boardOf` of
 * the project this map resolves — or of `null`, for an unfiled task — is that
 * board. `status: "all"`, so a task on an archived project still resolves one.
 */
export type TaskScope = "all" | "unfiled";

export interface TaskList {
  tasks: DbTaskWithProject[];
  projects: DbProject[];
  /** The projects by id, for `boardOf`. An unfiled task simply misses. */
  projectById: Map<number, DbProject>;
  /** False until the first read lands — the difference between "no tasks" and
   *  "not read yet", which is a blank page against an empty state. */
  loaded: boolean;
  reload: () => void;
}

/** `null` reads nothing at all: the filed half of `TaskPage` takes its rows
 *  from `projectsStore` instead, and a hook cannot be called conditionally. */
export function useTaskList(scope: TaskScope | null): TaskList {
  const [tasks, setTasks] = useState<DbTaskWithProject[]>([]);
  const [projects, setProjects] = useState<DbProject[]>([]);
  const [loaded, setLoaded] = useState(false);

  // Reads land out of order under a burst of writes — a drag fires an event
  // while the previous read is still in flight — so only the newest one is
  // allowed to set state. The same guard `projectsStore.open` makes.
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
    const onUpdated = () => void load().catch((e) => console.error("read tasks failed", e));
    window.addEventListener(PROJECTS_UPDATED_EVENT, onUpdated);
    return () => window.removeEventListener(PROJECTS_UPDATED_EVENT, onUpdated);
  }, [load, scope]);

  const projectById = useMemo(
    () => new Map(projects.map((p) => [p.id, p])),
    [projects],
  );

  const reload = useCallback(() => {
    void load().catch((e) => console.error("read tasks failed", e));
  }, [load]);

  return { tasks, projects, projectById, loaded, reload };
}
