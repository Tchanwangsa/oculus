import { ArrowsClockwise, CheckCircle, WarningCircle } from "@phosphor-icons/react";
import { Progress } from "@/components/ui/progress";
import { displayCode } from "@/lib/format/format";
import { cn } from "@/lib/utils";
import { useSyncStore } from "@/stores/sync/syncStore";
import { useFirstSync } from "./firstSync";

/** Canvas's phase names as the Sync page words them. */
const PHASE: Record<string, string> = {
  home: "overview",
  announcements: "announcements",
  modules: "modules",
  assignments: "assignments",
  ed: "Ed threads",
  ed_lessons: "Ed lessons",
};

/**
 * The sync's progress as one slim line, for the steps after Library: the
 * subject and phase in flight with a bar and its count, then how the run
 * setup started ended. Renders nothing before setup asks for a sync, unless
 * one is already running.
 */
export function SyncProgressLine({ className }: { className?: string }) {
  const scraping = useSyncStore((s) => s.scraping);
  const progress = useSyncStore((s) => s.progress);
  const completedAt = useSyncStore((s) => s.completedAt);
  const syncError = useSyncStore((s) => s.error);
  const { started, baseline, error: startError } = useFirstSync();

  const row = cn("flex min-w-0 items-center gap-2 text-xs text-muted-foreground", className);

  if (scraping) {
    const phase = progress?.phase ? (PHASE[progress.phase] ?? progress.phase) : null;
    return (
      <div className={row} role="status">
        <ArrowsClockwise size={13} className="shrink-0 animate-spin" />
        <span className="min-w-0 truncate">
          {progress?.course
            ? `Syncing ${displayCode(progress.course)}${phase ? ` — ${phase}` : ""}`
            : "Syncing your subjects…"}
        </span>
        {progress && progress.total > 0 && (
          <>
            <Progress
              value={(progress.done / progress.total) * 100}
              className="ml-auto h-1 w-20 shrink-0"
            />
            <span className="shrink-0 tabular-nums">
              {progress.done}/{progress.total}
            </span>
          </>
        )}
      </div>
    );
  }

  if (!started) return null;

  if (startError) {
    return (
      <p data-selectable className={cn(row, "text-warning")}>
        <WarningCircle size={13} className="shrink-0" />
        <span className="min-w-0">The first sync didn't start: {startError}</span>
      </p>
    );
  }

  if (completedAt > baseline) {
    return syncError ? (
      <p data-selectable className={cn(row, "text-warning")}>
        <WarningCircle size={13} className="shrink-0" />
        <span className="min-w-0">The sync stopped: {syncError}</span>
      </p>
    ) : (
      <p className={row}>
        <CheckCircle size={13} weight="fill" className="shrink-0 text-success" />
        Sync finished.
      </p>
    );
  }

  return null;
}
