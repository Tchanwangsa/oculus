import { useMemo, type ReactNode } from "react";
import type { HarnessThread } from "@/lib/harness";
import { sqliteUtcToMs } from "@/lib/format/format";
import { threadRowStatus } from "../StatusPill";
import { SectionHead } from "./SectionHead";

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
 * The Status view, T3's frame: every open thread not at work newest first,
 * with no headings, then "Working · n" and `completed` folding at the
 * bottom. Those sit in a `margin-top: auto` block inside `SideNav`'s `fill`
 * column, so they rest at the column's foot while the list is short and
 * follow it when it is long.
 */
export function StatusBody({
  threads,
  runningIds,
  workingOpen,
  onWorkingOpen,
  collapsed,
  row,
  completed,
}: {
  threads: HarnessThread[];
  runningIds: Set<number>;
  workingOpen: boolean;
  onWorkingOpen: (open: boolean) => void;
  collapsed: boolean;
  row: (thread: HarnessThread) => ReactNode;
  /** The Completed fold, under Working. */
  completed: ReactNode;
}) {
  const { working, rest } = useMemo(() => splitWorking(threads, runningIds), [threads, runningIds]);
  return (
    <>
      <div className="flex flex-col gap-0.5">{rest.map(row)}</div>
      <div className="mt-auto pt-1">
        {working.length > 0 && (
          <section aria-label="Working">
            <SectionHead label="Working" count={working.length} open={workingOpen} onOpen={onWorkingOpen} collapsed={collapsed} />
            {workingOpen && <div className="flex flex-col gap-0.5">{working.map(row)}</div>}
          </section>
        )}
        {completed}
      </div>
    </>
  );
}
