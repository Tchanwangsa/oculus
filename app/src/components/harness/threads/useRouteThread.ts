import { useEffect, useState } from "react";
import { getThreadsByIds, type HarnessThread } from "@/lib/harness";
import { useHarnessStore } from "@/stores/chat/harnessStore";

/**
 * The thread a Chat tab's route names. The store lists open threads, so a
 * completed one (a restored tab, a ⌘-click from Completed or search) is read
 * by id and kept; `missing` only once that read finds nothing, so the page
 * walks back to a new chat for a deleted thread and never for a completed one.
 */
export function useRouteThread(activeId: number | null): { thread: HarnessThread | null; missing: boolean } {
  const thread = useHarnessStore((s) => (activeId == null ? null : s.threads.find((t) => t.id === activeId) ?? null));
  const loaded = useHarnessStore((s) => s.threadsLoaded);
  const [gone, setGone] = useState<number | null>(null);
  const absent = loaded && activeId != null && thread == null;
  const lookUp = absent && gone !== activeId;
  useEffect(() => {
    if (!lookUp || activeId == null) return;
    let live = true;
    getThreadsByIds([activeId])
      .then(([found]) => {
        if (!live) return;
        if (found) useHarnessStore.getState().keep(found);
        else setGone(activeId);
      })
      .catch((e) => {
        console.error("thread read failed", e);
        if (live) setGone(activeId);
      });
    return () => {
      live = false;
    };
  }, [lookUp, activeId]);
  return { thread, missing: absent && gone === activeId };
}
