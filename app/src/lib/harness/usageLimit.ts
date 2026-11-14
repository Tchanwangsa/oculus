import { fmtTime } from "@/lib/format/format";
import { parseErrorMeta } from "./meta";
import type { HarnessItem, RateWindow } from "./types";

/** "Resets in 3 hr 48 min" under a day out, else "Resets Tue 9:00 pm". */
export function fmtResets(unixSeconds: number | null, now = Date.now()): string | null {
  if (!unixSeconds) return null;
  const ms = unixSeconds * 1000;
  const left = ms - now;
  if (left <= 0) return null;
  if (left < 86_400_000) {
    const mins = Math.round(left / 60_000);
    if (mins < 60) return `Resets in ${Math.max(1, mins)} min`;
    const h = Math.floor(mins / 60);
    const m = mins % 60;
    return m ? `Resets in ${h} hr ${m} min` : `Resets in ${h} hr`;
  }
  const d = new Date(ms);
  return `Resets ${d.toLocaleDateString([], { weekday: "short" })} ${fmtTime(ms)}`;
}

/** A run the provider stopped at its usage limit: the limited error row, and
 *  the run it ended (the notice is dismissed per run). */
export interface UsageLimitHit {
  /** The run, or the error row's id for a turn with no run. */
  key: string;
  /** "Resets in 2 hr", or null when no window says. */
  resets: string | null;
}

/**
 * Whether the thread's latest turn ended at a usage limit: an error row
 * marked `limit` after the last question or continuation. When the reset is
 * known it is the fullest rate window's; a window already past resets says
 * nothing.
 */
export function usageLimitHit(items: HarnessItem[], windows: RateWindow[], now = Date.now()): UsageLimitHit | null {
  for (let i = items.length - 1; i >= 0; i--) {
    const item = items[i];
    if (item.kind === "user" || item.kind === "continuation") return null;
    if (item.kind !== "error" || !parseErrorMeta(item).limit) continue;
    const fullest = [...windows].sort((a, b) => b.used_percent - a.used_percent)[0];
    return {
      key: item.run_id != null ? `run:${item.run_id}` : `row:${item.id}`,
      resets: fullest ? fmtResets(fullest.resets_at, now) : null,
    };
  }
  return null;
}
