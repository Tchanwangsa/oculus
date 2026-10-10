import {
  durationMinutes,
  isSelfImposed,
  type CalEvent,
  type CalKind,
} from "@/lib/planning/calendar";

/** `Extract`, so a renamed {@link CalKind} breaks here too. */
export type StripKind = Extract<CalKind, "due" | "note" | "task">;

export const STRIP_LABEL: Record<StripKind, string> = {
  due: "Due",
  note: "Notes",
  task: "Tasks",
};

export const HOUR_PX = 46;
export const GUTTER = "3.25rem";
/** Narrowest the week draws (gutter + 7×96px); below it the view scrolls
 *  sideways, since a narrower column cannot hold even a truncated title. */
export const MIN_GRID_PX = 724;
export const FULL_DAY_KEY = "calendar-full-day";
/** Deadline marker height, and its inset from the grid's edges. */
export const MARKER_PX = 16;
export const MARKER_INSET = 2;
/** The shortest a class block is drawn, however little time it covers. */
export const BLOCK_MIN_PX = 16;

/** Subject-hue tint for an instant's pill: self-imposed items (notes, tasks)
 *  and past ones are washed out beside a real deadline. */
export function tintPct(e: CalEvent, gone: boolean, full: number): number {
  const base = isSelfImposed(e) ? full * 0.6 : full;
  return Math.round(gone ? base * 0.6 : base);
}

/** A class's span in epoch ms, for `packLanes`. */
export function classSpan(e: CalEvent) {
  const start = e.start.getTime();
  return { start, end: start + durationMinutes(e) * 60_000 };
}
