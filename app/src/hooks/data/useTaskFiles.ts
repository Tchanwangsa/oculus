import { useCallback, useEffect, useRef, useState } from "react";
import { getTaskFiles, PROJECTS_UPDATED_EVENT, type DbTaskFile } from "@/lib/planning/projects";
import { useWindowEvent } from "@/hooks/backend/useEvents";

export interface TaskFiles {
  files: DbTaskFile[];
  /** False until the first read for this task lands. */
  loaded: boolean;
}

/** The files linked to a task, re-read on `PROJECTS_UPDATED_EVENT`. `null`
 *  reads nothing, since a hook cannot be called conditionally. */
export function useTaskFiles(taskId: number | null): TaskFiles {
  const [state, setState] = useState<{ taskId: number | null; files: DbTaskFile[] }>({
    taskId: null,
    files: [],
  });
  const run = useRef(0);

  const load = useCallback(() => {
    const token = ++run.current;
    if (taskId == null) return;
    getTaskFiles(taskId)
      .then((files) => {
        if (run.current === token) setState({ taskId, files });
      })
      .catch((e) => console.error("read task files failed", e));
  }, [taskId]);

  useEffect(load, [load]);
  useWindowEvent(PROJECTS_UPDATED_EVENT, load);

  // Files read for another task must not show under this one.
  const current = taskId != null && state.taskId === taskId;
  return { files: current ? state.files : [], loaded: taskId == null || current };
}
