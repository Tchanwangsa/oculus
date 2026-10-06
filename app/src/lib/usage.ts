import { getDb } from "@/lib/db";
import type { UsageKind } from "@/lib/usageContext";

// ── App usage: the Home Activity card ────────────────────────────────────────
//
// Rust keeps `usage_hours` (one row per local hour: seconds the window was open,
// and seconds of those with recent input or playing media), fed by the
// `usage_activity` pings from `useActivityPing`. This module reads it per day
// and sums the card's window; everything but the loaders is pure.

/** One local day's totals, in seconds. */
export interface UsageDay {
  open: number;
  active: number;
}

/** Days the chart and its totals cover, today included. */
export const CHART_DAYS = 30;

/** Days read back from today, so a streak can run past the chart. */
const HISTORY_DAYS = 366;

/** Local `YYYY-MM-DD`, the same prefix `usage_hours.hour` carries. */
export function dayKey(d: Date): string {
  const m = String(d.getMonth() + 1).padStart(2, "0");
  const day = String(d.getDate()).padStart(2, "0");
  return `${d.getFullYear()}-${m}-${day}`;
}

/** `d` shifted by whole days. Calendar arithmetic, so a DST change never
 *  lands a bar on the wrong date. */
function addDays(d: Date, n: number): Date {
  return new Date(d.getFullYear(), d.getMonth(), d.getDate() + n);
}

/** Per-day totals for the last `HISTORY_DAYS`; empty on a database that
 *  predates the table. */
export async function loadUsageDays(today: Date): Promise<Map<string, UsageDay>> {
  const db = await getDb();
  let rows: { day: string; open: number; active: number }[];
  try {
    rows = await db.select(
      `SELECT substr(hour, 1, 10) AS day,
              SUM(open_seconds) AS open,
              SUM(active_seconds) AS active
       FROM usage_hours
       WHERE hour >= $1
       GROUP BY day`,
      [dayKey(addDays(today, -HISTORY_DAYS))],
    );
  } catch (e) {
    if (String(e).includes("no such table")) return new Map();
    throw e;
  }
  return new Map(rows.map((r) => [r.day, { open: r.open ?? 0, active: r.active ?? 0 }]));
}

export interface ChartDay {
  key: string;
  date: Date;
}

/** The chart's days, oldest first, ending today — or, with `windowsBack`,
 *  that many whole windows earlier, for the comparison figures. */
export function chartDays(today: Date, windowsBack = 0): ChartDay[] {
  const end = addDays(today, -windowsBack * CHART_DAYS);
  return Array.from({ length: CHART_DAYS }, (_, i) => {
    const date = addDays(end, i - CHART_DAYS + 1);
    return { key: dayKey(date), date };
  });
}

/** Open and active totals over `range`, and how many of its days saw use. */
export function rangeTotals(
  days: Map<string, UsageDay>,
  range: ChartDay[],
): UsageDay & { daysUsed: number } {
  const sum = { open: 0, active: 0, daysUsed: 0 };
  for (const { key } of range) {
    const d = days.get(key);
    if (!d) continue;
    sum.open += d.open;
    sum.active += d.active;
    if (d.active > 0) sum.daysUsed++;
  }
  return sum;
}

/** Consecutive active days ending today — or yesterday, so a streak isn't
 *  broken before today's first use. */
export function usageStreak(days: Map<string, UsageDay>, today: Date): number {
  const active = (d: Date) => (days.get(dayKey(d))?.active ?? 0) > 0;
  let d = active(today) ? today : addDays(today, -1);
  let n = 0;
  while (active(d)) {
    n++;
    d = addDays(d, -1);
  }
  return n;
}

/** The longest run of consecutive active days in `days`. */
export function bestStreak(days: Map<string, UsageDay>): number {
  const keys = [...days].filter(([, d]) => d.active > 0).map(([k]) => k).sort();
  let best = 0;
  let run = 0;
  let prev: Date | null = null;
  for (const key of keys) {
    const [y, m, d] = key.split("-").map(Number);
    const date = new Date(y, m - 1, d);
    run = prev && dayKey(addDays(prev, 1)) === key ? run + 1 : 1;
    best = Math.max(best, run);
    prev = date;
  }
  return best;
}

/** Active seconds per day used, or 0 with none used. */
export function perDayUsed(t: { active: number; daysUsed: number }): number {
  return t.daysUsed ? t.active / t.daysUsed : 0;
}

/** Whole-percent change from `before` to `now`; null with nothing before to
 *  compare against. */
export function percentChange(now: number, before: number): number | null {
  if (before <= 0) return null;
  return Math.round(((now - before) / before) * 100);
}

/** Hour step for the chart's gridlines: the smallest that fits `maxSeconds`
 *  in four lines or fewer. */
export function hourStep(maxSeconds: number): number {
  const hours = maxSeconds / 3600;
  for (const step of [0.5, 1, 2, 3, 4, 6]) if (hours <= step * 4) return step;
  return 8;
}

/** "45m", "2h 15m", "3h"; whole hours past ten, where minutes are noise. */
export function fmtUsage(seconds: number): string {
  const mins = Math.round(seconds / 60);
  if (mins === 0) return seconds > 0 ? "<1m" : "0m";
  if (mins < 60) return `${mins}m`;
  const h = Math.floor(mins / 60);
  const m = mins % 60;
  if (h >= 10 || m === 0) return `${h}h`;
  return `${h}h ${m}m`;
}

// ── Active time by subject and type ──────────────────────────────────────────
//
// `usage_context_hours` splits each hour's active seconds by what was in front
// (`usageContext`), which the card's Subject and Type views stack per day.

/** One day's active seconds in one kind on one subject (0 = no subject). */
export interface UsageContextRow {
  day: string;
  kind: UsageKind;
  subjectId: number;
  active: number;
}

/** The chart window's rows; empty on a database that predates the table. */
export async function loadUsageContext(today: Date): Promise<UsageContextRow[]> {
  const db = await getDb();
  let rows: { day: string; kind: UsageKind; subject_id: number; active: number }[];
  try {
    rows = await db.select(
      `SELECT substr(hour, 1, 10) AS day, kind, subject_id,
              SUM(active_seconds) AS active
       FROM usage_context_hours
       WHERE hour >= $1
       GROUP BY day, kind, subject_id`,
      [dayKey(addDays(today, 1 - CHART_DAYS))],
    );
  } catch (e) {
    if (String(e).includes("no such table")) return [];
    throw e;
  }
  return rows.map((r) => ({ day: r.day, kind: r.kind, subjectId: r.subject_id, active: r.active ?? 0 }));
}

/** The Type view's groups, in stack order (bottom first). */
export const TYPE_GROUPS = ["lectures", "files", "course", "chat", "browser", "other"] as const;
export type TypeGroup = (typeof TYPE_GROUPS)[number];

/** The stored kind's display group; a kind this build doesn't know is "other". */
export function typeGroup(kind: string): TypeGroup {
  switch (kind) {
    case "lecture": return "lectures";
    case "file":
    case "document": return "files";
    case "course": return "course";
    case "chat": return "chat";
    case "browser": return "browser";
    default: return "other";
  }
}

/** Subject series past this many fold into {@link OTHER_SUBJECTS}. */
export const SUBJECT_SERIES = 5;
/** Series key for subjects folded past {@link SUBJECT_SERIES}. */
export const OTHER_SUBJECTS = "other";
/** Series key for time outside any subject (`subject_id` 0). */
export const NO_SUBJECT_SERIES = "none";

/** Active seconds per day, split into series. */
export interface StackedDays {
  /** Keys of the series with time in the range, in stack order (bottom first). */
  series: string[];
  /** Seconds per series for each day with time, aligned with `series`. */
  days: Map<string, number[]>;
  /** The largest daily total. */
  max: number;
}

/** Sums `rows` inside `range` per day and series; `order` ranks the keys. */
function stack(
  rows: UsageContextRow[],
  range: ChartDay[],
  keyOf: (r: UsageContextRow) => string,
  order: (a: string, b: string) => number,
): StackedDays {
  const inRange = new Set(range.map((d) => d.key));
  const sums = new Map<string, Map<string, number>>();
  const keys = new Set<string>();
  for (const r of rows) {
    if (!inRange.has(r.day) || r.active <= 0) continue;
    const key = keyOf(r);
    keys.add(key);
    const day = sums.get(r.day) ?? new Map<string, number>();
    day.set(key, (day.get(key) ?? 0) + r.active);
    sums.set(r.day, day);
  }
  const series = [...keys].sort(order);
  const days = new Map<string, number[]>();
  let max = 0;
  for (const [day, byKey] of sums) {
    const values = series.map((k) => byKey.get(k) ?? 0);
    days.set(day, values);
    max = Math.max(max, values.reduce((a, b) => a + b, 0));
  }
  return { series, days, max };
}

/** The Type view: kinds folded into {@link TYPE_GROUPS}, in that order. */
export function stackByType(rows: UsageContextRow[], range: ChartDay[]): StackedDays {
  const rank = (k: string) => TYPE_GROUPS.indexOf(k as TypeGroup);
  return stack(rows, range, (r) => typeGroup(r.kind), (a, b) => rank(a) - rank(b));
}

/**
 * The Subject view: one series per subject id (as a string), ordered by id so
 * a segment keeps its place day to day. Past {@link SUBJECT_SERIES} subjects
 * the smallest over the range fold into {@link OTHER_SUBJECTS}; time outside a
 * subject is {@link NO_SUBJECT_SERIES}. Those two stack last, in that order.
 */
export function stackBySubject(
  rows: UsageContextRow[],
  range: ChartDay[],
  keep = SUBJECT_SERIES,
): StackedDays {
  const inRange = new Set(range.map((d) => d.key));
  const totals = new Map<number, number>();
  for (const r of rows) {
    if (r.subjectId === 0 || !inRange.has(r.day) || r.active <= 0) continue;
    totals.set(r.subjectId, (totals.get(r.subjectId) ?? 0) + r.active);
  }
  // Ties go to the lower id, so the fold is deterministic.
  const kept = new Set(
    [...totals].sort((a, b) => b[1] - a[1] || a[0] - b[0]).slice(0, keep).map(([id]) => id),
  );
  const tail = [OTHER_SUBJECTS, NO_SUBJECT_SERIES];
  const rank = (k: string) => (tail.includes(k) ? Infinity : Number(k));
  return stack(
    rows,
    range,
    (r) => (r.subjectId === 0 ? NO_SUBJECT_SERIES : kept.has(r.subjectId) ? String(r.subjectId) : OTHER_SUBJECTS),
    (a, b) => rank(a) - rank(b) || tail.indexOf(a) - tail.indexOf(b),
  );
}

// ── Activity pings ───────────────────────────────────────────────────────────

/** How often `useActivityPing` reports each kind. */
export const PING_INTERVAL_MS = 30_000;

/** A leading-edge gate: true at most once per `intervalMs`. `now` is
 *  injectable for tests. */
export function createThrottle(intervalMs: number, now: () => number = Date.now): () => boolean {
  let last = -Infinity;
  return () => {
    const t = now();
    if (t - last < intervalMs) return false;
    last = t;
    return true;
  };
}
