import { useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { Button } from "@/components/ui/button";
import { Progress } from "@/components/ui/progress";
import { SyncProgressDetails } from "@/components/onboarding/library/SyncProgressLine";
import { useSyncStore } from "@/stores/sync/syncStore";
import { useParseStore } from "@/stores/sync/parseStore";
import { navigateActive } from "@/lib/shell/tabRouters";
import { jobProgress, jobProgressEvents, matchesJobProgressEvent, type JobProgressSources } from "./jobProgress.ts";
import { object, type Row } from "./model";

const EMPTY_SYNC: JobProgressSources["sync"] = { runId: null, scraping: false, progress: null };
const EMPTY_PARSES: JobProgressSources["parses"] = { statuses: {}, jobs: {}, failures: {} };

function elapsed(start: unknown, finish: unknown, now: number): string | null {
  if (typeof start !== "string") return null;
  const duration = Math.max(0, Math.floor(((typeof finish === "string" ? Date.parse(finish) : now) - Date.parse(start)) / 1000));
  if (!Number.isFinite(duration)) return null;
  return duration >= 3600 ? `${Math.floor(duration / 3600)}h ${Math.floor(duration % 3600 / 60)}m` : `${Math.floor(duration / 60)}m ${duration % 60}s`;
}

export function JobProgress({ kind, state, error }: { kind: string; state: Row; error?: string }) {
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
  const duration = job.interrupted ? null : elapsed(job.started_at, job.finished_at, now);
  const message = error ?? (job.phase === "error" ? String(job.error ?? "The job could not finish.") : running ? progress.error ?? cancelError : undefined);
  const href = typeof job.run_id === "number" ? `/sync?run=${job.run_id}` : null;
  const cancel = async () => {
    if (!progress.cancel) return;
    setCancelling(true);
    setCancelError(undefined);
    try { await invoke(progress.cancel.command, progress.cancel.args); }
    catch (cause) { setCancelError(String(cause)); setCancelling(false); }
  };
  return <div className="space-y-2 border-t border-border-subtle px-4 py-3 text-xs text-muted-foreground">
    {running && (progress.sync ? <SyncProgressDetails progress={progress.sync} /> : <div role="status" className="flex min-w-0 flex-wrap items-center gap-2">
      <span className="min-w-0 break-words">{progress.label}</span>
      {progress.percent != null && <><Progress value={progress.percent} className="ml-auto h-1 w-20 shrink-0" /><span className="tabular-nums">{Math.floor(progress.percent)}%</span></>}
      {progress.total != null && progress.total > 0 && <span className="tabular-nums">{progress.done ?? 0}/{progress.total}{kind === "Reparse" ? " files" : ""}</span>}
    </div>)}
    {job.phase === "complete" && <p>{typeof job.result === "string" ? job.result : "Job completed."}</p>}
    {message && <p role="alert" data-selectable className="break-words text-destructive">{message}</p>}
    {(duration || href || running && progress.cancel) && <div className="flex flex-wrap items-center gap-3">
      {duration && <span className="tabular-nums">{duration}{running ? " elapsed" : " total"}</span>}
      {href && <button type="button" data-tab-href={href} className="cursor-pointer text-brand hover:underline" onClick={() => navigateActive(href)}>View in Sync history</button>}
      {running && progress.cancel && <Button size="xs" variant="ghost" className="ml-auto" disabled={cancelling} onClick={() => void cancel()}>{cancelling ? "Cancelling…" : "Cancel"}</Button>}
    </div>}
  </div>;
}
