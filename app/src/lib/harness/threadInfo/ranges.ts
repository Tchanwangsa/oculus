import { fmtClockSecs } from "@/lib/lectures/media/clock";

/** 1-based, inclusive; `to` null is "to the end" (`oculus read -p 30-`). */
export interface PageRange {
  from: number;
  to: number | null;
}

/** `12`, `12-15`, `12,14,20-22` or `30-`, as `oculus read --pages` and
 *  Claude's Read `pages` take them. Anything unreadable is skipped. */
export function parsePages(spec: string | null | undefined): PageRange[] {
  if (!spec) return [];
  const out: PageRange[] = [];
  for (const part of spec.split(",")) {
    const m = /^\s*(\d+)\s*(?:[-–]\s*(\d*)\s*)?$/.exec(part);
    if (!m) continue;
    const from = Number(m[1]);
    if (from < 1) continue;
    if (m[2] === undefined) out.push({ from, to: from });
    else if (m[2] === "") out.push({ from, to: null });
    else out.push({ from: Math.min(from, Number(m[2])), to: Math.max(from, Number(m[2])) });
  }
  return out;
}

/** Ranges sorted, with overlapping and touching ones joined (12–15 and 16
 *  read as 12–16). */
export function mergePages(ranges: PageRange[]): PageRange[] {
  const sorted = [...ranges].sort((a, b) => a.from - b.from);
  const out: PageRange[] = [];
  for (const r of sorted) {
    const last = out[out.length - 1];
    if (last && (last.to === null || r.from <= last.to + 1)) {
      if (last.to !== null) last.to = r.to === null ? null : Math.max(last.to, r.to);
    } else {
      out.push({ ...r });
    }
  }
  return out;
}

/** `p. 12`, `pp. 12–19`, `pp. 3, 12–19`, `pp. 30–`; empty with no pages. */
export function formatPages(ranges: PageRange[]): string {
  const merged = mergePages(ranges);
  if (merged.length === 0) return "";
  const one = merged.length === 1 && merged[0].to === merged[0].from;
  const parts = merged.map((r) => (r.to === r.from ? `${r.from}` : `${r.from}–${r.to ?? ""}`));
  return `${one ? "p." : "pp."} ${parts.join(", ")}`;
}

/** A lecture's moments: `12:20`, or the span they cover, `12:20–31:05`. */
export function formatTimes(times: number[]): string {
  if (times.length === 0) return "";
  const lo = Math.min(...times);
  const hi = Math.max(...times);
  return lo === hi ? fmtClockSecs(lo) : `${fmtClockSecs(lo)}–${fmtClockSecs(hi)}`;
}
