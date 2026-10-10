import { CaretRight } from "@phosphor-icons/react";
import { cn } from "@/lib/utils";
import { Badge } from "@/components/ui/badge";
import { Progress } from "@/components/ui/progress";
import { INTERRUPTED_SYNC_ERROR, type SyncRunSummary } from "@/lib/db";
import { fmtClock, fmtDuration, sqliteUtcToMs } from "@/lib/format/format";
import type { SyncProgress } from "@/stores/sync/syncStore";
import { COLS } from "@/components/sync/history/constants";
import { FileCounts } from "@/components/sync/history/FileCounts";
import { RunDetail } from "@/components/sync/history/RunDetail";

export function RunRow({
  run,
  progress,
  expanded,
  onToggle,
}: {
  run: SyncRunSummary;
  /** Live progress, only for the run currently scraping. */
  progress: SyncProgress | null;
  expanded: boolean;
  onToggle: () => void;
}) {
  const startedMs = sqliteUtcToMs(run.started_at);
  const finishedMs = sqliteUtcToMs(run.finished_at);
  const running = run.status === "running";
  // Reconciled runs get finished_at at the next launch, so show no duration.
  const interrupted = run.error === INTERRUPTED_SYNC_ERROR;

  return (
    <div>
      <div
        role="button"
        tabIndex={0}
        onClick={onToggle}
        onKeyDown={(e) => e.key === "Enter" && onToggle()}
        className={cn(COLS, "py-2.5 cursor-pointer hover:bg-surface/60 transition-colors")}
      >
        <CaretRight
          size={9}
          className={cn(
            "shrink-0 text-muted-foreground/50 transition-transform",
            expanded && "rotate-90",
          )}
        />

        <span className="text-xs text-foreground truncate">
          {run.origin === "scheduled" ? "Scheduled" : "Manual"} run at{" "}
          <span className="tabular-nums">{fmtClock(startedMs, true)}</span>
        </span>

        <span className="text-[11px] text-muted-foreground tabular-nums">
          {running ? "running" : interrupted ? "—" : fmtDuration(startedMs, finishedMs)}
        </span>

        <span className="text-[11px] text-muted-foreground tabular-nums">
          {run.subjects_synced > 0
            ? `${run.subjects_synced} subject${run.subjects_synced === 1 ? "" : "s"}`
            : "—"}
        </span>

        <FileCounts run={run} />

        <div className="justify-self-end flex items-center gap-2 min-w-0">
          {running && progress ? (
            <div className="flex items-center gap-2">
              <Progress
                value={progress.total ? (progress.done / progress.total) * 100 : 0}
                className="h-1 w-16"
              />
              <Badge className="text-[11px]">Running</Badge>
            </div>
          ) : (
            <Badge
              variant={
                run.status === "completed"
                  ? "success"
                  : interrupted
                    ? "warning"
                    : run.status === "failed"
                      ? "destructive"
                      : "default"
              }
              className="text-[11px]"
            >
              {run.status === "completed"
                ? "Completed"
                : interrupted
                  ? "Interrupted"
                  : run.status === "failed"
                    ? "Failed"
                    : "Running"}
            </Badge>
          )}
        </div>
      </div>

      {expanded && <RunDetail run={run} />}
    </div>
  );
}
