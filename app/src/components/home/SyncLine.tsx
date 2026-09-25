import { useCallback, useState } from "react";
import { getSyncRunSummaries, type SyncRun } from "@/lib/db";
import { fmtSynced, sqliteUtcToMs } from "@/lib/format";
import { cn } from "@/lib/utils";
import { useHomeSection } from "./useHomeSection";

/** No event reports a finished sync here; mount and tab-front re-reads suffice. */
const EVENTS: string[] = [];

/** Past this, a new announcement or moved due date may already be missed. */
const STALE_MS = 3 * 24 * 60 * 60 * 1000;

/**
 * When the library last synced, under the date heading; one quiet colour
 * change when stale (sync is manual). Renders nothing before the first run.
 */
export function SyncLine() {
  const [run, setRun] = useState<SyncRun | null>(null);

  const reload = useCallback(() => {
    // The top `sync_runs` row may be running or interrupted; find a completed one.
    getSyncRunSummaries(5)
      .then((runs) => setRun(runs.find((r) => r.status === "completed" && r.finished_at) ?? null))
      .catch(console.error);
  }, []);

  useHomeSection(reload, EVENTS);

  if (!run) return null;

  // SQLite's `datetime('now')` has no zone; see `sqliteUtcToMs`.
  const finished = sqliteUtcToMs(run.finished_at);
  const stale = finished != null && Date.now() - finished > STALE_MS;

  return (
    <p className={cn("mt-1.5 text-[13px]", stale ? "text-warning" : "text-muted-foreground")}>
      {fmtSynced(run.finished_at)}
    </p>
  );
}
