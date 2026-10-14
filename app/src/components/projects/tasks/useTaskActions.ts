import { useCallback, useMemo } from "react";
import type { DbTaskWithProject } from "@/lib/planning/projects";
import { createTaskTag } from "@/lib/planning/projects";
import { useProjectsStore } from "@/stores/planning/projectsStore";
import { appendNeighbour } from "./universalTasks";
import type { CellActions } from "../lists/TaskCells";

/** Status, tags and refiling shared by every table that renders task rows. */
export function useTaskActions(tasks: DbTaskWithProject[]): CellActions {
  const moveTask = useProjectsStore((s) => s.moveTask);
  const updateTask = useProjectsStore((s) => s.updateTask);
  const refileTask = useProjectsStore((s) => s.refileTask);
  /** A drag or a status pick. Appends to the destination column (there is no
   *  manual order here), computed against the whole list, not the filtered one:
   *  a hidden neighbour is still a neighbour for `position`. */
  const move = useCallback(
    (task: DbTaskWithProject, columnId: string) => {
      const before = appendNeighbour(tasks, task, columnId);
      moveTask(task.id, columnId, before, null).catch((e) => console.error("move task failed", e));
    },
    [moveTask, tasks],
  );

  /** From the table's Project cell; the whole write is `refileTask`'s. */
  const refile = useCallback(
    (task: DbTaskWithProject, projectId: number | null) => {
      refileTask(task.id, projectId).catch((e) => console.error("refile task failed", e));
    },
    [refileTask],
  );

  const actions = useMemo(
    () => ({
      onMove: move,
      onRefile: refile,
      onTags: (task: DbTaskWithProject, tagIds: number[]) => {
        updateTask(task.id, { tagIds }).catch((e) => console.error("update task tags failed", e));
      },
      onCreateTag: (name: string) => createTaskTag(name),
    }),
    [move, refile, updateTask],
  );

  return actions;
}
