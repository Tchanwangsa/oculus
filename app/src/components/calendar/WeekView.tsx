import { useEffect, useMemo, useRef, useState } from "react";
import { cn } from "@/lib/utils";
import { useNow } from "@/hooks/ui/useNow";
import {
  groupEventsByDay,
  hourRange,
  isInstant,
  minutesFromMidnight,
  weekDays,
  type CalEvent,
} from "@/lib/planning/calendar";
import { sameDay, startOfDay } from "@/lib/format/format";
import { packLanes } from "@/lib/planning/lanes";
import { EventMark } from "./EventMark";
import { ClassBlock } from "@/components/calendar/week/ClassBlock";
import {
  FULL_DAY_KEY,
  GUTTER,
  HOUR_PX,
  MIN_GRID_PX,
  STRIP_LABEL,
  classSpan,
  type StripKind,
} from "@/components/calendar/week/constants";
import { InstantMarker } from "@/components/calendar/week/InstantMarker";
import { NowLine } from "@/components/calendar/week/NowLine";
import { PastWash } from "@/components/calendar/week/PastWash";
import { StripPill } from "@/components/calendar/week/StripPill";

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
                  {(stripDueByDay.get(startOfDay(d).getTime()) ?? []).map((e) => (
                    <StripPill key={e.id} event={e} today={today} colors={colors} />
                  ))}
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

                    {laid.map(({ item: event, lane, of }) => (
                      <ClassBlock
                        key={event.id}
                        event={event}
                        lane={lane}
                        of={of}
                        fromHour={fromHour}
                        gridHeight={gridHeight}
                        today={today}
                        colors={colors}
                      />
                    ))}

                    {/* Instants land at their own time, over the classes. */}
                    {(eventsByDay.get(startOfDay(day).getTime()) ?? [])
                      .filter(placeable)
                      .map((e) => (
                        <InstantMarker
                          key={e.id}
                          event={e}
                          fromHour={fromHour}
                          gridHeight={gridHeight}
                          today={today}
                          colors={colors}
                        />
                      ))}
                  </div>
                );
              })}
        </div>
      </div>
    </div>
  );
}
