import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { getTaskTags, PROJECTS_UPDATED_EVENT, type DbTaskTag } from "@/lib/planning/projects";
import { useWindowEvent } from "@/hooks/backend/useEvents";

export interface TaskTags {
  /** The registry, in order. */
  tags: DbTaskTag[];
  byId: Map<number, DbTaskTag>;
  /** False until the first read lands, so "not read yet" is not "no tags". */
  loaded: boolean;
}

/** The tag registry, re-read on `PROJECTS_UPDATED_EVENT` (a tag edit, or the
 *  agent's write through the CLI). */
export function useTaskTags(): TaskTags {
  const [tags, setTags] = useState<DbTaskTag[]>([]);
  const [loaded, setLoaded] = useState(false);
  // Only the newest read sets state when a burst of writes lands out of order.
  const run = useRef(0);

  const load = useCallback(() => {
    const token = ++run.current;
    getTaskTags()
      .then((rows) => {
        if (run.current !== token) return;
        setTags(rows);
        setLoaded(true);
      })
      .catch((e) => console.error("read task tags failed", e));
  }, []);

  useEffect(load, [load]);
  useWindowEvent(PROJECTS_UPDATED_EVENT, load);

  const byId = useMemo(() => new Map(tags.map((t) => [t.id, t])), [tags]);
  return { tags, byId, loaded };
}
