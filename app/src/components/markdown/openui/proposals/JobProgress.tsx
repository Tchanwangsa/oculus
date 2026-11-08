import { useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { Button } from "@/components/ui/button";
import { useSyncStore } from "@/stores/sync/syncStore";
import { useParseStore } from "@/stores/sync/parseStore";
import { navigateActive } from "@/lib/shell/tabRouters";
import { cn } from "@/lib/utils";
import { CardCallout } from "./CardFrame";
import { jobProgress, jobProgressEvents, matchesJobProgressEvent, type JobProgressSources } from "./jobProgress.ts";
import { object, type Row } from "./model";

const EMPTY_SYNC: JobProgressSources["sync"] = { runId: null, scraping: false, progress: null };
const EMPTY_PARSES: JobProgressSources["parses"] = { statuses: {}, jobs: {}, failures: {} };

function elapsed(start: unknown, finish: unknown, now: number): string | null {
  if (typeof start !== "string") return null;
  const duration = Math.max(0, Math.floor(((typeof finish === "string" ? Date.parse(finish) : now) - Date.parse(start)) / 1000));
  if (!Number.isFinite(duration)) return null;
  return duration >= 3600 ? `${Math.floor(duration / 3600)}h ${Math.floor(duration % 3600 / 60)}m` : duration >= 60 ? `${Math.floor(duration / 60)}m ${duration % 60}s` : `${duration}s`;
}

/** A job's live progress from its own sources (the sync and parse stores,
 * its tagged events), its elapsed time ticking while it runs, and Cancel. */
export function useJobProgress(kind: string, state: Row) {
  const job = object(state.job);
  const running = job.phase === "running";
  const [now, setNow] = useState(Date.now);
  const [event, setEvent] = useState<JobProgressSources["event"]>();
  const [cancelError, setCancelError] = useState<string>();
  const [cancelling, setCancelling] = useState(false);
  const current = useRef(state);
  current.current = state;
  const sync = useSyncStore((source) => running && kind === "Sync" ? source : EMPTY_SYNC);
  const parses = useParseStore((source) => running && kind === "Reparse" ? source : EMPTY_PARSES);
  useEffect(() => {
    if (!running) return;
    const timer = setInterval(() => setNow(Date.now()), 1000);
    return () => clearInterval(timer);
  }, [running]);
  useEffect(() => {
    if (!running) return;
    let active = true;
    const pending = jobProgressEvents(kind).map((name) => listen<Row>(name, ({ payload }) => {
      if (active && object(current.current.job).phase === "running" && matchesJobProgressEvent(kind, current.current, name, payload)) setEvent({ name, payload });
    }));
    return () => { active = false; for (const subscription of pending) void subscription.then((off) => off()).catch(() => {}); };
  }, [kind, running]);
  const progress = jobProgress(kind, state, { sync, parses, event });
  const cancel = async () => {
    if (!progress.cancel) return;
    setCancelling(true);
    setCancelError(undefined);
    try { await invoke(progress.cancel.command, progress.cancel.args); }
    catch (cause) { setCancelError(String(cause)); setCancelling(false); }
  };
  return {
    job, running, progress, cancel, cancelling,
    duration: job.interrupted ? null : elapsed(job.started_at, job.finished_at, now),
    /** A live failure the job has not ended on yet: a parse refused, or Cancel. */
    error: running ? progress.error ?? cancelError : undefined,
    href: typeof job.run_id === "number" ? `/sync?run=${job.run_id}` : null,
  };
}

/** A full-width bar: filled to the percent, or a sweep while the job gives none. */
function Bar({ percent }: { percent?: number }) {
  return <div role="progressbar" aria-valuemin={0} aria-valuemax={100} aria-valuenow={percent != null ? Math.floor(percent) : undefined} className="h-1 w-full overflow-hidden rounded-full bg-surface-raised">
    {percent != null ? <div className="h-full rounded-full bg-primary transition-[width] duration-500 ease-out" style={{ width: `${Math.max(2, percent)}%` }} /> : <div className="progress-sweep h-full w-full" />}
  </div>;
}

/**
 * The body of a started job: while it runs, what it is doing with its counts
 * and a bar, then its elapsed time, the Sync history link and Cancel; once it
 * ends, the result and how long it took. A failure leads, in the card's callout.
 */
export function JobProgress({ run, error }: { run: ReturnType<typeof useJobProgress>; error?: string }) {
  const { job, running, progress, duration, href, cancel, cancelling } = run;
  const figures = [progress.total != null && progress.total > 0 ? `${progress.done ?? 0} of ${progress.total}` : null, progress.percent != null ? `${Math.floor(progress.percent)}%` : null].filter(Boolean).join(" · ");
  const result = job.phase === "complete" ? typeof job.result === "string" && job.result ? job.result : "Finished." : null;
  const meta = duration || href || (running && progress.cancel);
  if (!running && !result && !meta && !error) return null;
  return <div className="space-y-2 px-4 py-3">
    {error && <CardCallout className="mx-0 my-0">{error}</CardCallout>}
    {running && <>
      <div role="status" className="flex min-w-0 items-baseline gap-3 text-[13px] leading-5">
        <span className="min-w-0 flex-1 truncate">{progress.label}</span>
        {figures && <span className="shrink-0 text-xs text-muted-foreground tabular-nums">{figures}</span>}
      </div>
      <Bar percent={progress.percent} />
    </>}
    {result && <div className="text-[13px] leading-5 break-words">{result}</div>}
    {meta && <div className={cn("flex min-h-6 flex-wrap items-center gap-x-3 gap-y-1 text-xs text-muted-foreground", running && "-mb-1")}>
      {duration && <span className="tabular-nums">{running ? `${duration} elapsed` : `Took ${duration}`}</span>}
      {href && <button type="button" data-tab-href={href} className="cursor-pointer hover:text-brand hover:underline" onClick={() => navigateActive(href)}>View in Sync history</button>}
      {running && progress.cancel && <Button size="xs" variant="ghost" className="-mr-2 ml-auto text-muted-foreground" disabled={cancelling} onClick={() => void cancel()}>{cancelling ? "Cancelling…" : "Cancel"}</Button>}
    </div>}
  </div>;
}
