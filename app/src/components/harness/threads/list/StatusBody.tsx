import { useMemo, type ReactNode } from "react";
import { CaretRight } from "@phosphor-icons/react";
import { SIDE_NAV_FOLDS } from "@/components/ui/layout/SideNav";
import type { HarnessThread } from "@/lib/harness";
import { sqliteUtcToMs } from "@/lib/format/format";
import { cn } from "@/lib/utils";
import { threadRowStatus } from "../StatusPill";

/** Newest first; equal times keep the list's order (`sort` is stable). */
export function byNewest(threads: HarnessThread[]): HarnessThread[] {
  return threads.slice().sort((a, b) => (sqliteUtcToMs(b.updated_at) ?? 0) - (sqliteUtcToMs(a.updated_at) ?? 0));
}

/** Splits the list into the threads at work and the rest, each newest first.
 *  A working thread waiting on the student (Approval, Question) is not at
 *  work: it stays in place by time. */
export function splitWorking(threads: HarnessThread[], runningIds: Set<number>) {
  const working: HarnessThread[] = [];
  const rest: HarnessThread[] = [];
  for (const t of byNewest(threads)) {
    (threadRowStatus(t, runningIds.has(t.id)) === "working" ? working : rest).push(t);
  }
  return { working, rest };
}

/**
 * The Status view, T3's frame: every thread not at work newest first, with
 * no headings, then "Working · n" folding at the bottom. That section sits in
 * a `margin-top: auto` block inside `SideNav`'s `fill` column, so it rests at
 * the column's foot while the list is short and follows it when it is long.
 */
export function StatusBody({
  threads,
  runningIds,
  workingOpen,
  onWorkingOpen,
  collapsed,
  row,
}: {
  threads: HarnessThread[];
  runningIds: Set<number>;
  workingOpen: boolean;
  onWorkingOpen: (open: boolean) => void;
  collapsed: boolean;
  row: (thread: HarnessThread) => ReactNode;
}) {
  const { working, rest } = useMemo(() => splitWorking(threads, runningIds), [threads, runningIds]);
  return (
    <>
      <div className="flex flex-col gap-0.5">{rest.map(row)}</div>
      <div className="mt-auto pt-2.5">
        {working.length > 0 && (
          <section aria-label="Working">
            <button
              type="button"
              aria-expanded={workingOpen}
              tabIndex={collapsed ? -1 : undefined}
              onClick={() => onWorkingOpen(!workingOpen)}
              className={cn(
                "flex w-full items-center gap-2 px-2.5 pt-1.5 pb-1.5 text-left text-[12.5px] font-medium text-muted-foreground transition-colors hover:text-foreground",
                SIDE_NAV_FOLDS,
              )}
            >
              <span className="flex-1">Working</span>
              <span className="font-normal tabular-nums opacity-80">{working.length}</span>
              <CaretRight
                size={12}
                className={cn("shrink-0 opacity-70 transition-transform will-change-transform", workingOpen && "rotate-90")}
              />
            </button>
            {workingOpen && <div className="flex flex-col gap-0.5">{working.map(row)}</div>}
          </section>
        )}
      </div>
    </>
  );
}
