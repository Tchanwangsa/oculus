import { useCallback } from "react";
import { useNavigate } from "react-router-dom";
import { taskHref } from "@/components/projects/nav/taskHref";
import { appendSlot, type TaskNode } from "@/components/projects/tasks/taskTree";
import type { DbProjectTask } from "@/lib/planning/projects";
import { useProjectsStore } from "@/stores/planning/projectsStore";

/** Every write this page makes, bound to the open task. */
export function useTaskMutations(task: DbProjectTask | null, nodes: TaskNode[]) {
  const navigate = useNavigate();
  const updateTask = useProjectsStore((s) => s.updateTask);
  const moveTask = useProjectsStore((s) => s.moveTask);
  const createTask = useProjectsStore((s) => s.createTask);
  const deleteTask = useProjectsStore((s) => s.deleteTask);
  const refileTask = useProjectsStore((s) => s.refileTask);

  const patch = useCallback(
    (next: Parameters<typeof updateTask>[1]) => {
      if (!task) return;
      updateTask(task.id, next).catch((e) => console.error("update task failed", e));
    },
    [task, updateTask],
  );

  /** Every column change on this page. `moveTask` is the only writer of
   *  `column_id`, `position` and `done_at`, so the status pill and the subtask
   *  checkboxes are the same call. */
  const move = useCallback(
    (which: number, columnId: string) => {
      const slot = appendSlot(nodes, columnId);
      moveTask(which, columnId, slot.before, slot.after).catch((e) =>
        console.error("move task failed", e),
      );
    },
    [moveTask, nodes],
  );

  /** Refile, then replace the route: a task's href is built from its project
   *  (`taskHref`), so the old path would read as "that task is gone". */
  const refile = useCallback(
    (projectId: number | null) => {
      if (!task) return;
      refileTask(task.id, projectId)
        .then(() => navigate(taskHref(projectId, task), { replace: true }))
        .catch((e) => console.error("refile task failed", e));
    },
    [navigate, refileTask, task],
  );

  return { patch, move, refile, createTask, deleteTask };
}
