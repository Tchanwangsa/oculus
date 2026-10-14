import { useCallback } from "react";
import { useProjectsStore } from "@/stores/planning/projectsStore";

/** `NewTaskButton`'s `onCreate`: files a task under a project or none. The
 *  list refreshes on `PROJECTS_UPDATED_EVENT`, not here. */
export function useCreateTask() {
  const createTask = useProjectsStore((s) => s.createTask);
  return useCallback(
    (projectId: number | null, title: string) => {
      // No `columnId`: `createTask` uses the destination's first column.
      createTask({ projectId, title }).catch((e) => console.error("create task failed", e));
    },
    [createTask],
  );
}
