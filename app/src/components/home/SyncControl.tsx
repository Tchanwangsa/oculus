import { useCallback, useEffect, useState } from "react";
import { ArrowsClockwise } from "@phosphor-icons/react";
import { Button } from "@/components/ui/button";
import { useAuth } from "@/hooks/sync/useAuth";
import { getSyncRunSummaries, type SyncRun } from "@/lib/db";
import { fmtSynced, sqliteUtcToMs } from "@/lib/format/format";
import { triggerSync } from "@/lib/pipeline/syncRunner";
import { useSyncStore } from "@/stores/sync/syncStore";
import { useHomeSection } from "./useHomeSection";

/** A finished run bumps `completedAt` instead; see the effect below. */
const EVENTS: string[] = [];

/** Past this, a new announcement or moved due date may already be missed. */
const STALE_MS = 3 * 24 * 60 * 60 * 1000;

/**
 * When the library last synced, with a Sync now pill — or the run's progress
 * while one is in flight. One quiet colour change when stale (sync is manual);
 * a refused start shows its reason inline.
 */
export function SyncControl() {
  // `undefined` until the first read, so "Never synced" can't flash.
  const [run, setRun] = useState<SyncRun | null | undefined>(undefined);
  const [error, setError] = useState<string | null>(null);
  const scraping = useSyncStore((s) => s.scraping);
  const progress = useSyncStore((s) => s.progress);
  const completedAt = useSyncStore((s) => s.completedAt);
  const { status: authStatus, connect } = useAuth();

  const reload = useCallback(() => {
    // The top `sync_runs` row may be running or interrupted; find a completed one.
    getSyncRunSummaries(5)
      .then((runs) => setRun(runs.find((r) => r.status === "completed" && r.finished_at) ?? null))
      .catch(console.error);
  }, []);

  useHomeSection(reload, EVENTS);

  useEffect(() => {
    if (completedAt) reload();
  }, [completedAt, reload]);

  const start = async () => {
    setError(null);
    // No live session: open the Canvas sign-in window, as the Sync page does.
    if (authStatus !== "connected") {
      connect();
      return;
    }
    try {
      await triggerSync();
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    }
  };

  if (run === undefined) return null;

  // SQLite's `datetime('now')` has no zone; see `sqliteUtcToMs`.
  const finished = sqliteUtcToMs(run?.finished_at ?? null);
  const stale = finished != null && Date.now() - finished > STALE_MS;

  return (
    <div className="flex flex-wrap items-center gap-x-3 gap-y-1 text-[13px]">
      <span className={stale ? "text-warning" : "text-muted-foreground"}>
        {fmtSynced(run?.finished_at ?? null)}
      </span>
      {scraping ? (
        <span className="inline-flex items-center gap-1.5 text-muted-foreground">
          <ArrowsClockwise size={12} className="animate-spin" />
          Syncing…
          {progress && progress.total > 0 && (
            <span className="tabular-nums">
              {progress.done}/{progress.total}
            </span>
          )}
        </span>
      ) : (
        <Button variant="outline" size="xs" onClick={start}>
          <ArrowsClockwise />
          Sync now
        </Button>
      )}
      {error && !scraping && <span className="text-warning/80">{error}</span>}
    </div>
  );
}
