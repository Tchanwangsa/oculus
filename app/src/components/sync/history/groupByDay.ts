import type { SyncRunSummary } from "@/lib/db";
import { fmtDayHeading, sqliteUtcToMs } from "@/lib/format/format";

/** Runs bucketed by local day; newest-first input keeps each day contiguous. */
export function groupByDay(runs: SyncRunSummary[]) {
  const groups: Array<{ key: string; heading: string; runs: SyncRunSummary[] }> = [];
  for (const run of runs) {
    const ms = sqliteUtcToMs(run.started_at) ?? 0;
    const d = new Date(ms);
    const key = `${d.getFullYear()}-${d.getMonth()}-${d.getDate()}`;
    const last = groups[groups.length - 1];
    if (last?.key === key) last.runs.push(run);
    else groups.push({ key, heading: fmtDayHeading(ms), runs: [run] });
  }
  return groups;
}
