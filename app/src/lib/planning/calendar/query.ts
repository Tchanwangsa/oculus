import { startOfDay } from "@/lib/format/format";
import { weekDays } from "./dates";
import { isInstant, type CalEvent } from "./model";

/** Buckets events by the local calendar day they start on. Reuse the result
 *  across grid cells instead of filtering the full event set for each day. */
export function groupEventsByDay(events: CalEvent[]): Map<number, CalEvent[]> {
  const byDay = new Map<number, CalEvent[]>();
  for (const event of events) {
    const day = startOfDay(event.start).getTime();
    const bucket = byDay.get(day);
    if (bucket) bucket.push(event);
    else byDay.set(day, [event]);
  }
  return byDay;
}

/** Its end (or instant) is behind `now` — what the calendar greys out. */
export function isPast(e: CalEvent, now: Date): boolean {
  return (e.end ?? e.start).getTime() < now.getTime();
}

/** Minutes from midnight — the y coordinate for a week-view block. */
export function minutesFromMidnight(d: Date): number {
  return d.getHours() * 60 + d.getMinutes();
}

/** A span with no `end_at` gets an hour, an instant half that, so it can still
 *  be drawn. */
export function durationMinutes(e: CalEvent): number {
  if (e.end == null) return isInstant(e) ? 30 : 60;
  return Math.max(15, Math.round((e.end.getTime() - e.start.getTime()) / 60_000));
}

/** Hour an event ends, from its own midnight; an end on a later day is 24, or
 *  a clock-time end past midnight would shrink the grid. */
function endHour(e: CalEvent): number {
  if (e.end == null) return e.start.getHours() + 1;
  if (startOfDay(e.end).getTime() > startOfDay(e.start).getTime()) return 24;
  return Math.ceil(minutesFromMidnight(e.end) / 60);
}

/**
 * The hour range a week grid covers: the events in view padded by an hour, at
 * least 8am–6pm. `includeHour` keeps a given hour (today's "now" line) on it.
 */
export function hourRange(
  events: CalEvent[],
  includeHour?: number,
): [number, number] {
  let lo = 8;
  let hi = 18;
  for (const e of events) {
    if (e.allDay) continue;
    lo = Math.min(lo, e.start.getHours());
    hi = Math.max(hi, endHour(e));
  }
  if (includeHour != null) {
    lo = Math.min(lo, includeHour);
    hi = Math.max(hi, includeHour + 1);
  }
  return [Math.max(0, lo - 1), Math.min(24, hi + 1)];
}

/** "PAR-148B-B1-B101-Kathleen Fitzpatrick Theatre" → "B101 · Kathleen
 *  Fitzpatrick Theatre": the room code leads, since a truncated prefix locates
 *  nothing. */
export function shortLocation(loc: string): string {
  const parts = loc.split("-");
  if (parts.length < 5) return loc;
  const codes = parts.slice(0, 4);
  if (!codes.every((c) => c.length > 0 && c.length <= 5 && /^[A-Za-z0-9]+$/.test(c))) {
    return loc;
  }
  return `${codes[3]} · ${parts.slice(4).join("-")}`;
}

export function fmtEventTime(e: CalEvent): string {
  if (e.allDay) return "All day";
  const t = (d: Date) =>
    d.toLocaleTimeString("en-AU", { hour: "numeric", minute: "2-digit" }).replace(" ", "");
  return e.end && !isInstant(e) ? `${t(e.start)}–${t(e.end)}` : t(e.start);
}

export function fmtMonth(d: Date): string {
  return d.toLocaleDateString("en-AU", { month: "long", year: "numeric" });
}

/** "11 – 17 Aug 2026", collapsing the month or year when both ends share it. */
export function fmtWeekRange(anchor: Date): string {
  const days = weekDays(anchor);
  const a = days[0];
  const b = days[6];
  const sameMonth = a.getMonth() === b.getMonth() && a.getFullYear() === b.getFullYear();
  const left = a.toLocaleDateString("en-AU", {
    day: "numeric",
    ...(sameMonth ? {} : { month: "short" }),
  });
  const right = b.toLocaleDateString("en-AU", {
    day: "numeric",
    month: "short",
    year: "numeric",
  });
  return `${left} – ${right}`;
}
