import { useState } from "react";
import { CaretRight } from "@phosphor-icons/react";
import { cn } from "@/lib/utils";
import type { SyncRunSummary } from "@/lib/db";
import { usePagedRows } from "@/components/ui/table/TablePagination";
import { GridTable, HeaderLabels } from "@/components/ui/table/GridTable";
import type { SyncProgress } from "@/stores/sync/syncStore";
import { COLS } from "@/components/sync/history/constants";
import { groupByDay } from "@/components/sync/history/groupByDay";
import { RunRow } from "@/components/sync/history/RunRow";

/**
 * One row per sync run, newest first, expanding into its changed files; the
 * full ledger (incl. skipped) is in a modal.
 */

/** Runs per page: a normal week fits on page one. */
const PAGE_SIZE = 25;

export function SyncHistoryTable({
  runs,
  progress,
}: {
  runs: SyncRunSummary[];
  progress: SyncProgress | null;
}) {
  const [expanded, setExpanded] = useState<Set<number>>(new Set());
  const [collapsedDays, setCollapsedDays] = useState<Set<string>>(new Set());
  const { page, pageCount, setPage, pageRows } = usePagedRows(runs, PAGE_SIZE);

  const toggle = (id: number) =>
    setExpanded((prev) => {
      const next = new Set(prev);
      next.has(id) ? next.delete(id) : next.add(id);
      return next;
    });

  const toggleDay = (key: string) =>
    setCollapsedDays((prev) => {
      const next = new Set(prev);
      next.has(key) ? next.delete(key) : next.add(key);
      return next;
    });

  return (
    <GridTable
      cols={COLS}
      header={
        <HeaderLabels
          labels={["", "Started", "Duration", "Subjects", "Files", "Status"]}
          endLast
        />
      }
      empty={runs.length === 0 && "No sync runs yet — pick your subjects above and run one."}
      pagination={{ page, pageCount, onPage: setPage, total: runs.length, unit: "sync run" }}
    >
      <div className="divide-y divide-border-subtle">
        {groupByDay(pageRows).map((group) => {
          const collapsed = collapsedDays.has(group.key);
          return (
            <div key={group.key}>
              <div
                role="button"
                tabIndex={0}
                onClick={() => toggleDay(group.key)}
                onKeyDown={(e) => e.key === "Enter" && toggleDay(group.key)}
                className={cn(
                  "flex items-center gap-2 px-5 py-1.5 bg-surface/70 cursor-pointer select-none hover:bg-surface transition-colors",
                  !collapsed && "border-b border-border-subtle",
                )}
              >
                <CaretRight
                  size={9}
                  className={cn(
                    "shrink-0 text-muted-foreground/50 transition-transform",
                    !collapsed && "rotate-90",
                  )}
                />
                <span className="text-[11px] font-medium text-muted-foreground">
                  {group.heading}
                </span>
                {collapsed && (
                  <span className="flex h-4 min-w-4 items-center justify-center rounded-full bg-muted-foreground/15 px-1 text-[10px] font-medium tabular-nums text-muted-foreground">
                    {group.runs.length}
                  </span>
                )}
              </div>
              {!collapsed && (
                <div className="divide-y divide-border-subtle">
                  {group.runs.map((run) => (
                    <RunRow
                      key={run.id}
                      run={run}
                      progress={run.status === "running" ? progress : null}
                      expanded={expanded.has(run.id)}
                      onToggle={() => toggle(run.id)}
                    />
                  ))}
                </div>
              )}
            </div>
          );
        })}
      </div>
    </GridTable>
  );
}
