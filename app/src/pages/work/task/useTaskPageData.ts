import { useEffect, useMemo, useState } from "react";
import { useParams } from "react-router-dom";
import { taskTree } from "@/components/projects/tasks/taskTree";
import { useTaskList } from "@/hooks/data/useTaskList";
import { PROJECTS_UPDATED_EVENT } from "@/lib/planning/projects";
import { useProjectsStore } from "@/stores/planning/projectsStore";
import { useWindowEvent } from "@/hooks/backend/useEvents";

/** The route's ids and the task, its project, parent and subtasks, kept loaded. */
export function useTaskPageData() {
  const { projectId, taskId } = useParams();
  // No project segment at all (not an unparseable one) means an unfiled task.
  const filed = projectId !== undefined;
  const id = Number(projectId);
  const tid = Number(taskId);

  const project = useProjectsStore((s) =>
    filed ? s.projects.find((p) => p.id === id) ?? null : null,
  );
  const storeProjects = useProjectsStore((s) => s.projects);
  const storeTasks = useProjectsStore((s) => s.tasks);
  const loadProjects = useProjectsStore((s) => s.loadProjects);
  const openProject = useProjectsStore((s) => s.open);
  const reload = useProjectsStore((s) => s.reload);

  // `null` reads nothing, so a filed task's page skips this query.
  const unfiled = useTaskList(filed ? null : "unfiled");
  const tasks = filed ? storeTasks : unfiled.tasks;
  // For the Project picker; both halves already hold the list at `status: "all"`.
  const projects = filed ? storeProjects : unfiled.projects;

  const [listed, setListed] = useState(false);
  /** Whether the read that would have found this task has landed ("gone" vs
   *  "not read yet"). */
  const ready = filed ? listed : unfiled.loaded;

  // `status: "all"`: a task of an archived project is still one you can open.
  useEffect(() => {
    if (!filed) return;
    setListed(false);
    loadProjects({ status: "all" }).finally(() => setListed(true));
  }, [loadProjects, id, filed]);

  useEffect(() => {
    if (filed && Number.isFinite(id)) void openProject(id);
  }, [openProject, id, filed]);

  // Every write (here, another tab, the agent via the CLI) arrives as this
  // event. The unfiled half has its own listener inside `useTaskList`.
  useWindowEvent(PROJECTS_UPDATED_EVENT, () => {
    if (filed) void reload();
  });

  const task = useMemo(() => tasks.find((t) => t.id === tid) ?? null, [tasks, tid]);
  const parent = useMemo(
    () => (task?.parent_id != null ? tasks.find((t) => t.id === task.parent_id) ?? null : null),
    [tasks, task?.parent_id],
  );
  const children = useMemo(
    () => (task ? tasks.filter((t) => t.parent_id === task.id) : []),
    [tasks, task],
  );
  // Only for the `moveTask` slots.
  const nodes = useMemo(() => taskTree(tasks), [tasks]);

  return { filed, id, tid, project, projects, ready, task, parent, children, nodes };
}
