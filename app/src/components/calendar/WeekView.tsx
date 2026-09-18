import { useEffect, useRef, useState } from "react";
import { cn } from "@/lib/utils";
import { useNow } from "@/hooks/useNow";
import {
  durationMinutes,
  eventsOn,
  fmtEventTime,
  hourRange,
  isInstant,
  isPast,
  isSelfImposed,
  minutesFromMidnight,
  sameDay,
  shortLocation,
  startOfDay,
  weekDays,
  type CalEvent,
  type CalKind,
} from "@/lib/calendar";
import { packLanes } from "@/lib/lanes";
import { EventMark } from "./EventMark";
import { EventPopover } from "./EventPopover";

/** The kinds the strip above the grid can be named after. `Extract` rather
 *  than a literal union, so a renamed {@link CalKind} breaks here too. */
type StripKind = Extract<CalKind, "due" | "note" | "task">;

const STRIP_LABEL: Record<StripKind, string> = {
  due: "Due",
  note: "Notes",
  task: "Tasks",
};

const HOUR_PX = 46;
const GUTTER = "3.25rem";
/**
 * The narrowest the week is drawn at, in pixels: the 52px hour gutter plus
 * seven 96px columns.
 *
 * Below this the view scrolls sideways instead of compressing further. A
 * column under about 96px cannot hold even a truncated class title — and it is
 * routinely halved again by `packLanes` when two classes overlap — so with the
 * side panel open every block read as an ellipsis. Seven columns of nothing
 * legible is worse than six columns and a nudge.
 */
const MIN_GRID_PX = 724;
const FULL_DAY_KEY = "calendar-full-day";
/** Height of a deadline marker, and the breathing room kept between it and
 *  the grid's own edges — see the clamp where they are positioned. */
const MARKER_PX = 16;
const MARKER_INSET = 2;
/** The shortest a class block is drawn, however little time it covers. */
const BLOCK_MIN_PX = 16;

/**
 * How much of the subject's hue an instant's pill is tinted with.
 *
 * A note or a task is washed out beside a deadline of the same colour: a
 * reminder you wrote, or a date you set yourself on a board, should not read as
 * loudly as a cutoff you will be marked against. Anything already past is
 * quieter again, like every other layer.
 */
function tintPct(e: CalEvent, gone: boolean, full: number): number {
  const base = isSelfImposed(e) ? full * 0.6 : full;
  return Math.round(gone ? base * 0.6 : base);
}

/** A class's span in epoch milliseconds, for {@link packLanes} — the only
 *  CalEvent-specific part of laying overlapping classes side by side. */
function classSpan(e: CalEvent) {
  const start = e.start.getTime();
  return { start, end: start + durationMinutes(e) * 60_000 };
}

/**
 * The timetable view: a Monday-first hour grid of classes and recordings, with
 * deadlines in a strip above it. Deadlines sit up there rather than in the grid
 * because most land at 11:59pm — inside the grid they would pin every week open
 * to midnight and read as a class that runs all evening.
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
  const days = weekDays(anchor);
  const today = useNow();
  const weekHasToday = days.some((d) => sameDay(d, today));

  const inWeek = events.filter((e) =>
    days.some((d) => sameDay(d, e.start)),
  );
  const timed = inWeek.filter((e) => !isInstant(e) && !e.allDay);
  // The fitted range covers every class in the week by construction, so
  // nothing can hide outside it — but "the grid decides which hours exist" is
  // a claim you have to take on trust, and midnight-to-6am being absent looks
  // like a limitation rather than a fit. The toggle makes the whole day
  // reachable on demand, and the choice sticks.
  const [fullDay, setFullDay] = useState(
    () => localStorage.getItem(FULL_DAY_KEY) === "1",
  );
  const fitted = hourRange(timed, weekHasToday ? today.getHours() : undefined);
  const [fromHour, toHour] = fullDay ? [0, 24] : fitted;
  const hours = Array.from({ length: toHour - fromHour }, (_, i) => fromHour + i);
  const gridHeight = (toHour - fromHour) * HOUR_PX;

  /**
   * A deadline is drawn at its hour when the grid covers that hour — a 9am
   * submission belongs at 9am, next to the class you would be in. The ones the
   * grid cannot place (an 11:59pm cutoff, with the grid stopping at 7pm) go to
   * the strip above instead. Each deadline appears in exactly one of the two,
   * so nothing is ever shown twice. A pinned note is placed the same way: it
   * is an instant too, and has no length to draw.
   */
  const placeable = (e: CalEvent) => {
    if (!isInstant(e) || e.allDay) return false;
    const m = minutesFromMidnight(e.start);
    return m >= fromHour * 60 && m <= toHour * 60;
  };

  const scroller = useRef<HTMLDivElement>(null);
  const stripDue = inWeek.filter((e) => (isInstant(e) || e.allDay) && !placeable(e));
  const hasDue = stripDue.length > 0;
  // The strip is named after the loudest layer in it. A week whose unplaceable
  // instants are all notes, or all tasks, would otherwise file them under
  // "Due", which is a claim about a deadline that does not exist. A real
  // deadline — or an all-day class, which has no quieter name — still wins the
  // heading whenever one is present.
  const stripKind: StripKind = stripDue.some((e) => e.kind === "due" || !isInstant(e))
    ? "due"
    : stripDue.some((e) => e.kind === "task")
      ? "task"
      : "note";

  // Open where the user actually is: "now", a couple of hours up so there is
  // context above it, on the week containing today — otherwise the start of
  // the teaching day. Keyed on the week rather than on `today`, or the
  // minute tick would yank the scroll back every sixty seconds.
  const weekKey = days[0].toDateString();
  useEffect(() => {
    const el = scroller.current;
    if (!el) return;
    const focusHour = weekHasToday ? new Date().getHours() - 2 : 8;
    el.scrollTop = Math.max(0, (focusHour - fromHour) * HOUR_PX - 8);
  }, [fromHour, weekKey, weekHasToday]);

  return (
    // One horizontal scroller around all three stacked grids — the day
    // headers, the deadline strip and the hour grid — because they share a
    // column template and have to stay aligned as it moves. The vertical
    // scroller stays inside, on the hours alone, so the headers hold still.
    <div className="h-full overflow-x-auto">
      <div
        className="flex h-full flex-col"
        style={{ minWidth: MIN_GRID_PX }}
      >
        {/* Day headers */}
        <div
          className="grid shrink-0 border-b border-border-subtle"
          style={{ gridTemplateColumns: `${GUTTER} repeat(7, minmax(0, 1fr))` }}
        >
          <div className="flex items-end justify-center pb-1.5">
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

        {/* Deadlines strip */}
        {hasDue && (
          <div
            className="grid shrink-0 border-b border-border-subtle bg-surface/40"
            style={{ gridTemplateColumns: `${GUTTER} repeat(7, minmax(0, 1fr))` }}
          >
            <div className="flex items-center justify-end gap-1 px-2 py-1.5 text-right text-[10px] font-medium text-muted-foreground">
              <EventMark kind={stripKind} color="currentColor" size={10} />
              {STRIP_LABEL[stripKind]}
            </div>
            {days.map((d) => (
              <div
                key={d.toISOString()}
                className="min-w-0 border-l border-border-subtle px-1 py-1 space-y-0.5"
              >
                {eventsOn(stripDue, d).map((e) => {
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

        {/* Hour grid */}
        <div ref={scroller} className="flex-1 min-h-0 overflow-y-auto">
          <div
            className="relative grid"
            style={{ gridTemplateColumns: `${GUTTER} repeat(7, minmax(0, 1fr))` }}
          >
            {/* Hour labels */}
            <div>
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

            {days.map((day) => {
              const laid = packLanes(
                eventsOn(timed, day).filter(
                  (e) => minutesFromMidnight(e.start) >= fromHour * 60,
                ),
                classSpan,
              );
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

                  {/* Elapsed time, washed grey. Drawn before the blocks so it
                      sits under them — each block carries its own past styling. */}
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
                    // Blocks are clamped into the grid the way the deadline
                    // markers below are, and for the same reason: Canvas
                    // publishes cutoff-shaped *classes* (an 11:59pm–11:59pm peer
                    // review), and one drawn at its own minute with the minimum
                    // block height hangs off the bottom edge of the card. A block
                    // with real length is shortened to the last row instead of
                    // moved, so 11pm–12:30am still starts at 11pm.
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
                    // A finished class keeps its shape but loses its subject
                    // colour, so the eye lands on what is still ahead.
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
                            // 18% of the subject's hue: a readable tint in light
                            // mode and a dark wash in dark mode, from one value.
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

                  {/* Deadlines and notes land on the grid at their own time, over
                      the classes rather than beside them — a submission is an
                      instant, not a block competing for the hour. */}
                  {eventsOn(inWeek, day)
                    .filter(placeable)
                    .map((e) => {
                      const gone = isPast(e, today);
                      const tone = gone
                        ? "var(--color-chart-other)"
                        : (colors.get(e.subjectId) ?? "");
                      // Centred on the exact minute, then pulled back inside
                      // the grid. Deadlines cluster at the edges of the day —
                      // 11:59pm is the common one — and a marker centred there
                      // hangs half off the bottom. Nudging by at most half its
                      // own height keeps it whole and still on the right hour,
                      // where snapping to 11pm or to midnight would move it to a
                      // time it is not due.
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
    </div>
  );
}

/**
 * The grey over what has already happened: a whole column for a day gone by,
 * the hours above the line on today, nothing ahead. `surface` sits only a
 * couple of percent off the page background, so the wash is mixed from the
 * muted foreground instead — the one neutral that reads in both themes.
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

/** The current time, drawn across today's column only — and only while "now"
 *  is inside the hours the grid actually covers, or it would float past the
 *  last row and over whatever sits below. */
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
