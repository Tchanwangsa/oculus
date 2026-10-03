import { useEffect, useMemo, useRef, useState } from "react";
import { cn } from "@/lib/utils";
import { useNow } from "@/hooks/useNow";
import {
  durationMinutes,
  fmtEventTime,
  groupEventsByDay,
  hourRange,
  isInstant,
  isPast,
  isSelfImposed,
  minutesFromMidnight,
  shortLocation,
  weekDays,
  type CalEvent,
  type CalKind,
} from "@/lib/calendar";
import { sameDay, startOfDay } from "@/lib/format";
import { packLanes } from "@/lib/lanes";
import { EventMark } from "./EventMark";
import { EventPopover } from "./EventPopover";

/** `Extract`, so a renamed {@link CalKind} breaks here too. */
type StripKind = Extract<CalKind, "due" | "note" | "task">;

const STRIP_LABEL: Record<StripKind, string> = {
  due: "Due",
  note: "Notes",
  task: "Tasks",
};

const HOUR_PX = 46;
const GUTTER = "3.25rem";
/** Narrowest the week draws (gutter + 7×96px); below it the view scrolls
 *  sideways, since a narrower column cannot hold even a truncated title. */
const MIN_GRID_PX = 724;
const FULL_DAY_KEY = "calendar-full-day";
/** Deadline marker height, and its inset from the grid's edges. */
const MARKER_PX = 16;
const MARKER_INSET = 2;
/** The shortest a class block is drawn, however little time it covers. */
const BLOCK_MIN_PX = 16;

/** Subject-hue tint for an instant's pill: self-imposed items (notes, tasks)
 *  and past ones are washed out beside a real deadline. */
function tintPct(e: CalEvent, gone: boolean, full: number): number {
  const base = isSelfImposed(e) ? full * 0.6 : full;
  return Math.round(gone ? base * 0.6 : base);
}

/** A class's span in epoch ms, for {@link packLanes}. */
function classSpan(e: CalEvent) {
  const start = e.start.getTime();
  return { start, end: start + durationMinutes(e) * 60_000 };
}

/**
 * The timetable: a Monday-first hour grid of classes, with deadlines in a strip
 * above it — most land at 11:59pm and would otherwise pin the grid to midnight.
 */
export function WeekView({
  anchor,
  events,
  colors,
}: {
  anchor: Date;
  events: CalEvent[];
  colors: Map<number, string>;
}) {
  const days = useMemo(() => weekDays(anchor), [anchor]);
  const eventsByDay = useMemo(() => groupEventsByDay(events), [events]);
  const today = useNow();
  const weekHasToday = days.some((d) => sameDay(d, today));

  const inWeek = useMemo(
    () => days.flatMap((day) => eventsByDay.get(startOfDay(day).getTime()) ?? []),
    [days, eventsByDay],
  );
  const timed = useMemo(() => inWeek.filter((e) => !isInstant(e) && !e.allDay), [inWeek]);
  // The fitted range covers every class; the toggle shows the whole day on
  // demand, and the choice sticks.
  const [fullDay, setFullDay] = useState(
    () => localStorage.getItem(FULL_DAY_KEY) === "1",
  );
  const fitted = hourRange(timed, weekHasToday ? today.getHours() : undefined);
  const [fromHour, toHour] = fullDay ? [0, 24] : fitted;
  const hours = Array.from({ length: toHour - fromHour }, (_, i) => fromHour + i);
  const gridHeight = (toHour - fromHour) * HOUR_PX;

  /**
   * An instant (deadline, pinned note) sits on the grid when it covers that
   * hour, otherwise in the strip above — never both.
   */
  const placeable = (e: CalEvent) => {
    if (!isInstant(e) || e.allDay) return false;
    const m = minutesFromMidnight(e.start);
    return m >= fromHour * 60 && m <= toHour * 60;
  };

  // Only the time styling changes each minute; event overlap geometry does not.
  const lanesByDay = useMemo(
    () => days.map((day) => packLanes(
      (eventsByDay.get(startOfDay(day).getTime()) ?? []).filter(
        (e) => !isInstant(e) && !e.allDay && minutesFromMidnight(e.start) >= fromHour * 60,
      ),
      classSpan,
    )),
    [days, eventsByDay, fromHour],
  );

  const scroller = useRef<HTMLDivElement>(null);
  const stripDue = inWeek.filter((e) => (isInstant(e) || e.allDay) && !placeable(e));
  const stripDueByDay = groupEventsByDay(stripDue);
  const hasDue = stripDue.length > 0;
  // Named after the loudest layer: "Due" only when a real deadline (or an
  // all-day class) is present.
  const stripKind: StripKind = stripDue.some((e) => e.kind === "due" || !isInstant(e))
    ? "due"
    : stripDue.some((e) => e.kind === "task")
      ? "task"
      : "note";

  // Open two hours above "now" on the current week, else at 8am. Keyed on the
  // week, not `today`, so the minute tick doesn't yank the scroll.
  const weekKey = days[0].toDateString();
  useEffect(() => {
    const el = scroller.current;
    if (!el) return;
    const focusHour = weekHasToday ? new Date().getHours() - 2 : 8;
    el.scrollTop = Math.max(0, (focusHour - fromHour) * HOUR_PX - 8);
  }, [fromHour, weekKey, weekHasToday]);

  return (
    // One scroller for both axes: headers stick to the top and the hour gutter
    // to the left. Sticky resolves against the nearest scrollport, so nested
    // scrollers would let the gutter slide away with the columns.
    <div ref={scroller} className="h-full overflow-auto">
      <div style={{ minWidth: MIN_GRID_PX }}>
        {/* Headers and strip stick together, so nothing measures the header. */}
        <div className="sticky top-0 z-40 bg-card">
          <div
            className="grid border-b border-border-subtle"
            style={{ gridTemplateColumns: `${GUTTER} repeat(7, minmax(0, 1fr))` }}
          >
            <div className="sticky left-0 z-10 flex items-end justify-center bg-card pb-1.5">
              <button
                type="button"
                onClick={() => {
                  const next = !fullDay;
                  setFullDay(next);
                  localStorage.setItem(FULL_DAY_KEY, next ? "1" : "0");
                }}
                title={
                  fullDay
                    ? "Fit the grid to the hours in use"
                    : "Show all 24 hours"
                }
                className="rounded px-1 py-0.5 text-[10px] font-medium text-muted-foreground/70 hover:bg-surface hover:text-foreground"
              >
                {fullDay ? "Fit" : "24h"}
              </button>
            </div>
            {days.map((d) => {
              const isToday = sameDay(d, today);
              return (
                <div key={d.toISOString()} className="px-2 py-1.5 text-center">
                  <div className="text-[10px] font-semibold uppercase tracking-wider text-muted-foreground">
                    {d.toLocaleDateString("en-AU", { weekday: "short" })}
                  </div>
                  <div
                    className={cn(
                      "mt-0.5 inline-flex h-5 min-w-5 items-center justify-center rounded-full px-1 text-[11px] tabular-nums",
                      isToday
                        ? "bg-primary font-semibold text-primary-foreground"
                        : "text-foreground",
                    )}
                  >
                    {d.getDate()}
                  </div>
                </div>
              );
            })}
          </div>

          {hasDue && (
            <div
              className="grid border-b border-border-subtle bg-surface/40"
              style={{ gridTemplateColumns: `${GUTTER} repeat(7, minmax(0, 1fr))` }}
            >
              {/* Opaque tint: the strip's pills pass underneath this cell. */}
              <div
                className="sticky left-0 z-10 flex items-center justify-end gap-1 px-2 py-1.5 text-right text-[10px] font-medium text-muted-foreground"
                style={{
                  backgroundColor:
                    "color-mix(in srgb, var(--color-surface) 40%, var(--color-card))",
                }}
              >
                <EventMark kind={stripKind} color="currentColor" size={10} />
                {STRIP_LABEL[stripKind]}
              </div>
              {days.map((d) => (
                <div
                  key={d.toISOString()}
                  className="min-w-0 border-l border-border-subtle px-1 py-1 space-y-0.5"
                >
                  {(stripDueByDay.get(startOfDay(d).getTime()) ?? []).map((e) => {
                      const gone = isPast(e, today);
                      const tone = gone
                        ? "var(--color-chart-other)"
                        : (colors.get(e.subjectId) ?? "");
                      return (
                        <EventPopover key={e.id} event={e} color={colors.get(e.subjectId) ?? ""}>
                          <button
                            type="button"
                            className="flex w-full min-w-0 items-center gap-1 rounded-[4px] border-l-2 px-1.5 py-0.5 text-left transition-colors hover:brightness-95 dark:hover:brightness-125"
                            style={{
                              borderLeftColor: tone,
                              backgroundColor: `color-mix(in srgb, ${tone} ${tintPct(
                                e,
                                gone,
                                20,
                              )}%, var(--color-card))`,
                            }}
                          >
                            <EventMark kind={e.kind} color={tone} size={9} />
                            {!e.allDay && (
                              <span className="shrink-0 text-[9.5px] tabular-nums leading-4 text-muted-foreground">
                                {fmtEventTime(e)}
                              </span>
                            )}
                            <span
                              className={cn(
                                "truncate text-[10.5px] font-medium leading-4",
                                gone ? "text-muted-foreground" : "text-foreground",
                              )}
                            >
                              {e.title}
                            </span>
                          </button>
                        </EventPopover>
                      );
                    })}
                </div>
              ))}
            </div>
          )}
        </div>

        <div
          className="relative grid"
          style={{ gridTemplateColumns: `${GUTTER} repeat(7, minmax(0, 1fr))` }}
        >
          {/* Above the markers' z-20 so blocks slide under the times. */}
          <div className="sticky left-0 z-30 bg-card">
            {hours.map((h) => (
              <div
                key={h}
                className="relative text-right pr-2"
                style={{ height: HOUR_PX }}
              >
                <span className="absolute -top-1.5 right-2 text-[10px] tabular-nums text-muted-foreground">
                  {h === 0 ? "" : `${h % 12 === 0 ? 12 : h % 12}${h < 12 ? "am" : "pm"}`}
                </span>
              </div>
            ))}
          </div>

              {days.map((day, dayIndex) => {
                const laid = lanesByDay[dayIndex];
                return (
                  <div
                    key={day.toISOString()}
                    className="relative border-l border-border-subtle"
                  >
                    {hours.map((h) => (
                      <div
                        key={h}
                        className="border-b border-border-subtle/60"
                        style={{ height: HOUR_PX }}
                      />
                    ))}

                    {/* Elapsed time, under the blocks. */}
                    <PastWash
                      day={day}
                      now={today}
                      fromHour={fromHour}
                      gridHeight={gridHeight}
                    />

                    {sameDay(day, today) && (
                      <NowLine fromHour={fromHour} toHour={toHour} />
                    )}

                    {laid.map(({ item: event, lane, of }) => {
                      // Clamped into the grid like the markers: Canvas publishes
                      // 11:59pm–11:59pm "classes" that would hang off the bottom.
                      // A block with real length is shortened, not moved.
                      const exact =
                        ((minutesFromMidnight(event.start) - fromHour * 60) / 60) * HOUR_PX;
                      const wanted = Math.max(
                        BLOCK_MIN_PX,
                        (durationMinutes(event) / 60) * HOUR_PX - 2,
                      );
                      const height = Math.max(
                        BLOCK_MIN_PX,
                        Math.min(wanted, gridHeight - exact),
                      );
                      const top = Math.max(0, Math.min(exact, gridHeight - height));
                      const color = colors.get(event.subjectId) ?? "";
                      // A finished class loses its subject colour.
                      const gone = isPast(event, today);
                      const tone = gone ? "var(--color-chart-other)" : color;
                      return (
                        <EventPopover key={event.id} event={event} color={color}>
                          <button
                            type="button"
                            className="absolute overflow-hidden rounded-[4px] border-l-2 px-1.5 py-0.5 text-left transition-colors hover:brightness-95 dark:hover:brightness-125"
                            style={{
                              top,
                              height,
                              left: `calc(${(lane / of) * 100}% + 2px)`,
                              width: `calc(${100 / of}% - 4px)`,
                              borderLeftColor: tone,
                              backgroundColor: `color-mix(in srgb, ${tone} ${gone ? 12 : 18}%, var(--color-card))`,
                            }}
                          >
                            <span
                              className={cn(
                                "block truncate text-[10.5px] font-medium leading-4",
                                gone ? "text-muted-foreground" : "text-foreground",
                              )}
                            >
                              {event.title}
                            </span>
                            {height > 30 && (
                              <span className="block truncate text-[10px] leading-3.5 text-muted-foreground">
                                {fmtEventTime(event)}
                                {event.location ? ` · ${shortLocation(event.location)}` : ""}
                              </span>
                            )}
                          </button>
                        </EventPopover>
                      );
                    })}

                    {/* Instants land at their own time, over the classes. */}
                    {(eventsByDay.get(startOfDay(day).getTime()) ?? [])
                      .filter(placeable)
                      .map((e) => {
                        const gone = isPast(e, today);
                        const tone = gone
                          ? "var(--color-chart-other)"
                          : (colors.get(e.subjectId) ?? "");
                        // Centred on the minute, then pulled back inside the grid
                        // (11:59pm would hang half off) — by at most half its height,
                        // so it stays on the right hour.
                        const exact =
                          ((minutesFromMidnight(e.start) - fromHour * 60) / 60) * HOUR_PX;
                        const top = Math.min(
                          Math.max(exact - MARKER_PX / 2, MARKER_INSET),
                          Math.max(
                            MARKER_INSET,
                            gridHeight - MARKER_PX - MARKER_INSET,
                          ),
                        );
                        return (
                          <EventPopover
                            key={e.id}
                            event={e}
                            color={colors.get(e.subjectId) ?? ""}
                          >
                            <button
                              type="button"
                              className="absolute z-20 flex min-w-0 items-center gap-1 overflow-hidden rounded-[3px] border-l-2 px-1 text-left shadow-xs transition-colors hover:brightness-95 dark:hover:brightness-125"
                              style={{
                                top,
                                height: MARKER_PX,
                                left: 2,
                                right: 2,
                                borderLeftColor: tone,
                                backgroundColor: `color-mix(in srgb, ${tone} ${tintPct(
                                  e,
                                  gone,
                                  38,
                                )}%, var(--color-card))`,
                              }}
                            >
                              <EventMark kind={e.kind} color={tone} size={9} />
                              <span
                                className={cn(
                                  "truncate text-[10px] font-medium leading-4",
                                  gone ? "text-muted-foreground" : "text-foreground",
                                )}
                              >
                                {e.title}
                              </span>
                            </button>
                          </EventPopover>
                        );
                      })}
                  </div>
                );
              })}
        </div>
      </div>
    </div>
  );
}

/**
 * Grey over elapsed time. Mixed from muted-foreground, since `surface` is too
 * close to the page background to read in both themes.
 */
function PastWash({
  day,
  now,
  fromHour,
  gridHeight,
}: {
  day: Date;
  now: Date;
  fromHour: number;
  gridHeight: number;
}) {
  let height = 0;
  if (sameDay(day, now)) {
    const elapsed = (minutesFromMidnight(now) - fromHour * 60) / 60;
    height = Math.min(gridHeight, Math.max(0, elapsed * HOUR_PX));
  } else if (day.getTime() < startOfDay(now).getTime()) {
    height = gridHeight;
  }
  if (height <= 0) return null;
  return (
    <div
      className="pointer-events-none absolute inset-x-0 top-0"
      style={{
        height,
        backgroundColor:
          "color-mix(in srgb, var(--color-muted-foreground) 9%, transparent)",
      }}
    />
  );
}

/** The current time across today's column, only while inside the grid's hours. */
function NowLine({ fromHour, toHour }: { fromHour: number; toHour: number }) {
  const now = new Date();
  const mins = minutesFromMidnight(now);
  if (mins < fromHour * 60 || mins > toHour * 60) return null;
  const top = ((mins - fromHour * 60) / 60) * HOUR_PX;
  return (
    <div
      className="pointer-events-none absolute inset-x-0 z-10 border-t border-destructive"
      style={{ top }}
    >
      <span className="absolute -left-1 -top-[3px] block h-1.5 w-1.5 rounded-full bg-destructive" />
    </div>
  );
}
