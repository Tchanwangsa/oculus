import { DAY_MS, addDays, addMonths, fmtMonth, startOfWeek } from "@/lib/planning/calendar";
import { startOfDay } from "@/lib/format/format";
import type { TimelineZoom } from "./zoom";

export const PX_PER_DAY: Record<TimelineZoom, number> = { day: 52, week: 22, month: 5 };

const PAD_DAYS: Record<TimelineZoom, number> = { day: 2, week: 7, month: 20 };

/** Floor on the drawn span, like `hourRange`'s. */
const MIN_SPAN_DAYS: Record<TimelineZoom, number> = { day: 14, week: 56, month: 180 };

export const NAME_PX = 208;
export const ROW_PX = 30;
export const SUB_ROW_PX = 22;
export const BAR_PX = 13;
export const SUB_BAR_PX = 9;
/** Lane packing runs on these drawn pixels, not on times. */
export const MARKER_PX = 12;
export const MIN_BAR_PX = 10;

/** Everything dated, padded, at least the zoom's floor, and always including
 *  now. */
export function timelineRange(points: number[], now: Date, zoom: TimelineZoom): [Date, Date] {
  let lo = now.getTime();
  let hi = now.getTime();
  for (const p of points) {
    lo = Math.min(lo, p);
    hi = Math.max(hi, p);
  }

  let start = addDays(startOfDay(new Date(lo)), -PAD_DAYS[zoom]);
  let end = addDays(startOfDay(new Date(hi)), PAD_DAYS[zoom] + 1);

  const short = MIN_SPAN_DAYS[zoom] - Math.round((end.getTime() - start.getTime()) / DAY_MS);
  if (short > 0) {
    start = addDays(start, -Math.floor(short / 2));
    end = addDays(end, Math.ceil(short / 2));
  }

  // Snap to whole weeks/months so the first tick isn't a stub.
  if (zoom === "week") {
    start = startOfWeek(start);
    end = addDays(startOfWeek(end), 7);
  } else if (zoom === "month") {
    start = addMonths(start, 0);
    end = addMonths(end, 1);
  }
  return [start, end];
}

export interface Tick {
  at: Date;
  end: Date;
  label: string;
  /** A month boundary inside a day or week axis. */
  major: boolean;
  today: boolean;
}

export function ticksFor(zoom: TimelineZoom, start: Date, end: Date, now: Date): Tick[] {
  const out: Tick[] = [];
  const limit = end.getTime();
  if (zoom === "month") {
    for (let d = addMonths(start, 0); d.getTime() < limit; d = addMonths(d, 1)) {
      out.push({
        at: d,
        end: addMonths(d, 1),
        label: d.toLocaleDateString("en-AU", { month: "short" }),
        major: d.getMonth() === 0,
        today: false,
      });
    }
    return out;
  }
  const step = zoom === "week" ? 7 : 1;
  for (let d = start; d.getTime() < limit; d = addDays(d, step)) {
    out.push({
      at: d,
      end: addDays(d, step),
      label:
        zoom === "week"
          ? d.toLocaleDateString("en-AU", { day: "numeric", month: "short" })
          : String(d.getDate()),
      major: zoom === "week" ? false : d.getDate() === 1,
      today:
        zoom === "day" &&
        d.getFullYear() === now.getFullYear() &&
        d.getMonth() === now.getMonth() &&
        d.getDate() === now.getDate(),
    });
  }
  return out;
}

export interface Band {
  at: Date;
  end: Date;
  label: string;
}

/** The row above the ticks: months, or years over a month axis. */
export function bandsFor(zoom: TimelineZoom, start: Date, end: Date): Band[] {
  const out: Band[] = [];
  if (zoom === "month") {
    for (
      let y = new Date(start.getFullYear(), 0, 1);
      y.getTime() < end.getTime();
      y = new Date(y.getFullYear() + 1, 0, 1)
    ) {
      out.push({
        at: y,
        end: new Date(y.getFullYear() + 1, 0, 1),
        label: String(y.getFullYear()),
      });
    }
    return out;
  }
  for (let m = addMonths(start, 0); m.getTime() < end.getTime(); m = addMonths(m, 1)) {
    out.push({ at: m, end: addMonths(m, 1), label: fmtMonth(m) });
  }
  return out;
}
