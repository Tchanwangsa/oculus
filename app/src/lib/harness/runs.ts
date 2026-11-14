import { sqliteUtcToMs } from "@/lib/format/format";
import type { HarnessRun, RunStatus } from "./types";

/** A `harness_runs` row as `harnessLoad` returns it (`Run` in
 *  `harness/core/load.rs`); a run that never started reads its interval
 *  from its request. */
export interface RunRow {
  id: number;
  execution_status: string;
  delivery_state: string;
  answer_item_id: number | null;
  started_at: string | null;
  ended_at: string | null;
  stop_requested_at?: string | null;
}

/** The statuses of a run with a timeline row. `starting` is saved and not
 *  yet handed to its provider (`preparing` in rows from before it); a run
 *  that only waits, or was cancelled, is never loaded as a run. */
const STATUSES = new Set<RunStatus>(["preparing", "starting", "running", "completed", "interrupted", "failed"]);

export function runFromRow(row: RunRow): HarnessRun {
  const status = STATUSES.has(row.execution_status as RunStatus) ? (row.execution_status as RunStatus) : "failed";
  return {
    id: row.id,
    status,
    delivery: row.delivery_state,
    answer_item_id: row.answer_item_id,
    started_ms: sqliteUtcToMs(row.started_at) ?? null,
    ended_ms: sqliteUtcToMs(row.ended_at) ?? null,
    ...(row.stop_requested_at != null && row.ended_at == null ? { stopping: true } : {}),
  };
}

/** Whether a run has ended (settled by Rust, or closed by startup reconcile). */
export const runEnded = (run: HarnessRun | undefined) =>
  !!run && (run.status === "completed" || run.status === "interrupted" || run.status === "failed");

/** The thread's newest live run (going out, or being answered): the run a
 *  Stop names. Null with none. */
export function liveRunId(runs: Record<number, HarnessRun> | undefined): number | null {
  let latest: number | null = null;
  for (const run of Object.values(runs ?? {})) {
    const live = run.status === "running" || run.status === "starting" || run.status === "preparing";
    if (live && (latest == null || run.id > latest)) latest = run.id;
  }
  return latest;
}

/** When the thread's newest live run (going out, or being answered) started;
 *  null with none (a turn the agent started itself has no run). */
export function runningSince(runs: Record<number, HarnessRun> | undefined): number | null {
  let latest: HarnessRun | null = null;
  for (const run of Object.values(runs ?? {})) {
    const live = run.status === "running" || run.status === "starting";
    if (live && (!latest || run.id > latest.id)) latest = run;
  }
  return latest?.started_ms ?? null;
}

/** "12s", "1m 5s"; at least a second. */
export function elapsedText(ms: number): string {
  const s = Math.max(1, Math.round(ms / 1000));
  if (s < 60) return `${s}s`;
  const m = Math.floor(s / 60);
  return `${m}m${s % 60 ? ` ${s % 60}s` : ""}`;
}
