import { useEffect } from "react";
import { CaretRight } from "@phosphor-icons/react";
import { SIDE_NAV_FOLDS } from "@/components/ui/layout/SideNav";
import type { Subject } from "@/lib/db";
import type { HarnessThread } from "@/lib/harness";
import { completedKey, useCompletedStore } from "@/stores/chat/completedStore";
import { cn } from "@/lib/utils";
import { CompletedList } from "./CompletedList";

/** A subject group's last line, "n completed", folding open in place onto
 *  that subject's completed rows (its own paged read). */
export function SubjectCompleted({
  subjectId,
  open,
  onOpenFold,
  bySubject,
  activeId,
  collapsed,
  onOpen,
  onReopen,
}: {
  subjectId: number | null;
  open: boolean;
  onOpenFold: (open: boolean) => void;
  bySubject: Map<number, Subject>;
  activeId: number | null;
  collapsed: boolean;
  onOpen: (thread: HarnessThread) => void;
  onReopen: (id: number) => void;
}) {
  const key = completedKey(subjectId);
  const count = useCompletedStore((s) => s.counts[key] ?? 0);
  const fold = useCompletedStore((s) => s.bySubject[key]);
  const unread = fold == null;
  useEffect(() => {
    if (open && unread && count > 0) {
      useCompletedStore.getState().loadSubject(subjectId).catch((e) => console.error("completed threads read failed", e));
    }
  }, [open, unread, count, subjectId]);
  if (!count) return null;
  return (
    <>
      <button
        type="button"
        aria-expanded={open}
        tabIndex={collapsed ? -1 : undefined}
        onClick={() => onOpenFold(!open)}
        className={cn(
          "flex w-full items-center gap-1.5 px-2.5 pt-1 pb-0.5 text-left text-[11.5px] text-muted-foreground/80 transition-colors hover:text-foreground",
          SIDE_NAV_FOLDS,
        )}
      >
        <CaretRight
          size={11}
          className={cn("shrink-0 transition-transform will-change-transform", open && "rotate-90")}
        />
        {/* One child: the row's flex gap would split the words. */}
        <span>
          <span className="tabular-nums">{count}</span> completed
        </span>
      </button>
      {open && fold && (
        <CompletedList
          page={fold.page}
          bySubject={bySubject}
          showSubject={false}
          activeId={activeId}
          collapsed={collapsed}
          onOpen={onOpen}
          onReopen={onReopen}
          onMore={() =>
            useCompletedStore.getState().moreSubject(subjectId).catch((e) => console.error("completed threads read failed", e))
          }
        />
      )}
    </>
  );
}
