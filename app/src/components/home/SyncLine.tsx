import { useCallback, useState } from "react";
import { getSyncRunSummaries, type SyncRun } from "@/lib/db";
import { fmtSynced, sqliteUtcToMs } from "@/lib/format";
import { cn } from "@/lib/utils";
import { useHomeSection } from "./useHomeSection";

/** No event announces a finished sync to this line — the sidebar owns live
 *  progress — so mount and the tab's front edge are the whole story, which is
 *  exactly what `useHomeSection` is for. Empty, and module-level so the
 *  reference stays stable. */
const EVENTS: string[] = [];

/** Past three days a library is far enough behind that a new announcement or
 *  a moved due date could already have been missed. */
const STALE_MS = 3 * 24 * 60 * 60 * 1000;

/**
 * When the library was last brought up to date, under the date heading.
 *
 * Sync is manual-only, so staleness is a fact the student has to be told
 * rather than something the app is about to fix — but it is worth exactly one
 * quiet colour change and nothing more. No progress bar, no toast, no button:
 * live sync progress belongs in the sidebar, and the Sync page is where you go
 * to start one.
 *
 * Nothing renders until there is a run to describe. An empty `<p>` would hold
 * open a gap under the heading on a library that has never been synced, which
 * is the one moment the page has least to say.
 */
export function SyncLine() {
  const [run, setRun] = useState<SyncRun | null>(null);

  const reload = useCallback(() => {
    // A few rather than one: `sync_runs` is the only sync clock and an
    // interrupted or still-running row sits at the top of it, so asking for a
    // single row would leave this line blank for the length of a sync.
    getSyncRunSummaries(5)
      .then((runs) => setRun(runs.find((r) => r.status === "completed" && r.finished_at) ?? null))
      .catch(console.error);
  }, []);

  useHomeSection(reload, EVENTS);

  if (!run) return null;

  // Through `sqliteUtcToMs`, like every other stamp in the app: SQLite's
  // `datetime('now')` carries no zone marker and `new Date` would read it as
  // local, which on this side of the world is a ten-hour lie.
  const finished = sqliteUtcToMs(run.finished_at);
  const stale = finished != null && Date.now() - finished > STALE_MS;

  return (
    <p className={cn("mt-1.5 text-[13px]", stale ? "text-warning" : "text-muted-foreground")}>
      {fmtSynced(run.finished_at)}
    </p>
  );
}
