import { useEffect } from "react";
import type { Subject } from "@/lib/db";
import type { HarnessThread } from "@/lib/harness";
import { useCompletedStore } from "@/stores/chat/completedStore";
import { SectionHead } from "../list/SectionHead";
import { CompletedList } from "./CompletedList";

/** Every completed thread, a total of the per-subject counts. */
export const completedTotal = (counts: Record<string, number>): number =>
  Object.values(counts).reduce((sum, n) => sum + n, 0);

/** The Status view's "Completed · n" fold under Working: grey one-line rows,
 *  ten at first, read when it first opens. Nothing while there are none. */
export function StatusCompleted({
  open,
  onOpenFold,
  bySubject,
  activeId,
  collapsed,
  onOpen,
  onReopen,
}: {
  open: boolean;
  onOpenFold: (open: boolean) => void;
  bySubject: Map<number, Subject>;
  activeId: number | null;
  collapsed: boolean;
  onOpen: (thread: HarnessThread) => void;
  onReopen: (id: number) => void;
}) {
  const total = useCompletedStore((s) => completedTotal(s.counts));
  const page = useCompletedStore((s) => s.all);
  const unread = page == null;
  useEffect(() => {
    if (open && unread) useCompletedStore.getState().loadAll().catch((e) => console.error("completed threads read failed", e));
  }, [open, unread]);
  if (!total) return null;
  return (
    <section aria-label="Completed">
      <SectionHead label="Completed" count={total} open={open} onOpen={onOpenFold} collapsed={collapsed} />
      {open && page && (
        <CompletedList
          page={page}
          bySubject={bySubject}
          showSubject
          activeId={activeId}
          collapsed={collapsed}
          onOpen={onOpen}
          onReopen={onReopen}
          onMore={() => useCompletedStore.getState().moreAll().catch((e) => console.error("completed threads read failed", e))}
        />
      )}
    </section>
  );
}
