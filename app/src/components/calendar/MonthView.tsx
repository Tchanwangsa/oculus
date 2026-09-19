import { useEffect, useRef, useState } from "react";
import { cn } from "@/lib/utils";
import { useNow } from "@/hooks/useNow";
import {
  eventsOn,
  fmtEventTime,
  isInstant,
  isPast,
  isSelfImposed,
  monthGrid,
  sameDay,
  startOfDay,
  type CalEvent,
} from "@/lib/calendar";
import {
  Popover,
  PopoverContent,
  PopoverTrigger,
} from "@/components/ui/popover";
import { EventMark } from "./EventMark";
import { EventPopover } from "./EventPopover";

const WEEKDAYS = ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"];

/**
 * A cell's fixed furniture, in pixels: the `pt-1` above the date, the date's
 * own 20px line, and the `gap-0.5` that follows it.
 */
const CELL_HEADER_PX = 26;
/** One chip: a 16px line inside `py-px`, plus the `gap-0.5` above it. */
const CHIP_PX = 20;
/**
 * How many chips a cell shows before the rest fold into "+N more", until the
 * grid has been measured — one paint's worth, and what a full-height window
 * fits anyway.
 *
 * It used to be the only answer, and it was wrong at every other size: four
 * chips need 106px of cell, so a window with the side panel open (or simply a
 * short one) drew the fourth sliced in half by the cell's `overflow-hidden`,
 * with no "+N more" to say anything was missing. The count is measured now.
 */
const VISIBLE_FALLBACK = 4;

export function MonthView({
  month,
  events,
  colors,
}: {
  month: Date;
  events: CalEvent[];
  colors: Map<number, string>;
}) {
  const weeks = monthGrid(month);
  const today = useNow();
  const todayStart = startOfDay(today).getTime();

  // How many chips a cell can actually hold, measured rather than assumed.
  // The six rows split the grid evenly, so one height answers for all of them;
  // the observer is what keeps it true through a window resize, a page zoom
  // and the side panel opening.
  const body = useRef<HTMLDivElement>(null);
  const [rowH, setRowH] = useState(0);
  useEffect(() => {
    const el = body.current;
    if (!el) return;
    const measure = () => setRowH(el.clientHeight / weeks.length);
    measure();
    const ro = new ResizeObserver(measure);
    ro.observe(el);
    return () => ro.disconnect();
  }, [weeks.length]);

  const capacity =
    rowH > 0
      ? Math.max(1, Math.floor((rowH - CELL_HEADER_PX) / CHIP_PX))
      : VISIBLE_FALLBACK;

  return (
    <div className="flex h-full flex-col">
      <div className="grid grid-cols-7 border-b border-border-subtle">
        {WEEKDAYS.map((d) => (
          <div
            key={d}
            className="px-2 py-1.5 text-[10px] font-semibold uppercase tracking-wider text-muted-foreground"
          >
            {d}
          </div>
        ))}
      </div>

      {/* Six equal rows: the grid keeps one height for every month, so paging
          through the year never makes the page jump. */}
      <div ref={body} className="grid flex-1 min-h-0 grid-rows-6">
        {weeks.map((week, wi) => (
          <div key={wi} className="grid grid-cols-7 border-b border-border-subtle last:border-b-0">
            {week.map((day) => {
              const inMonth = day.getMonth() === month.getMonth();
              const dayEvents = eventsOn(events, day);
              const isToday = sameDay(day, today);
              // A day already behind us reads grey; today and everything after
              // keeps the subject colours.
              const dayGone = day.getTime() < todayStart;
              const visible =
                dayEvents.length > capacity ? Math.max(1, capacity - 1) : capacity;
              return (
                <div
                  key={day.toISOString()}
                  className={cn(
                    "min-w-0 border-r border-border-subtle last:border-r-0 px-1 pt-1 pb-0.5 overflow-hidden flex flex-col gap-0.5",
                    !inMonth && "bg-surface/40",
                  )}
                >
                  <div className="flex items-center px-1">
                    <span
                      className={cn(
                        "text-[11px] leading-5 tabular-nums",
                        isToday
                          ? "flex h-5 min-w-5 items-center justify-center rounded-full bg-primary px-1 font-semibold text-primary-foreground"
                          : inMonth
                            ? "text-foreground"
                            : "text-muted-foreground/50",
                      )}
                    >
                      {day.getDate()}
                    </span>
                  </div>

                  {/* One chip's worth of the cell goes to "+N more" when
                      there is a remainder — the link is shorter than a chip,
                      so trading one for it always fits. */}
                  {dayEvents.slice(0, visible).map((e) => (
                    <Chip
                      key={e.id}
                      event={e}
                      color={colors.get(e.subjectId) ?? ""}
                      gone={dayGone || isPast(e, today)}
                    />
                  ))}

                  {dayEvents.length > visible && (
                    <MoreLink
                      day={day}
                      events={dayEvents}
                      colors={colors}
                      hidden={dayEvents.length - visible}
                    />
                  )}
                </div>
              );
            })}
          </div>
        ))}
      </div>
    </div>
  );
}

/** "11a" — the coarsest useful time, since a month cell has room for one word
 *  before the title. Minutes appear only when they are not on the hour. */
function chipTime(d: Date): string {
  const h = d.getHours();
  const m = d.getMinutes();
  const hour = h % 12 === 0 ? 12 : h % 12;
  return `${hour}${m === 0 ? "" : `:${String(m).padStart(2, "0")}`}${h < 12 ? "a" : "p"}`;
}

function Chip({
  event,
  color,
  gone,
}: {
  event: CalEvent;
  color: string;
  gone: boolean;
}) {
  // A deadline, a note or a task is an instant with no shape of its own, so it
  // is tinted to stand out of a column of classes; a class stays flat, or a
  // busy day turns into stripes. What you set yourself — a note, a task — is
  // tinted more faintly than a deadline a course set for you.
  const instant = isInstant(event);
  const tone = gone ? "var(--color-chart-other)" : color;
  const fill = isSelfImposed(event) ? (gone ? 8 : 12) : gone ? 12 : 20;
  return (
    <EventPopover event={event} color={color}>
      <button
        type="button"
        className={cn(
          "group flex w-full min-w-0 items-center gap-1 rounded-sm px-1 py-px text-left transition-colors",
          instant ? "font-medium" : "hover:bg-surface",
        )}
        style={
          instant
            ? {
                backgroundColor: `color-mix(in srgb, ${tone} ${fill}%, var(--color-card))`,
              }
            : undefined
        }
      >
        <EventMark kind={event.kind} color={tone} size={9} />
        {!event.allDay && (
          <span
            className={cn(
              "shrink-0 text-[10px] tabular-nums leading-4",
              gone ? "text-muted-foreground/60" : "text-muted-foreground",
            )}
          >
            {chipTime(event.start)}
          </span>
        )}
        <span
          className={cn(
            "truncate text-[10.5px] leading-4",
            gone ? "text-muted-foreground" : "text-foreground",
          )}
        >
          {event.title}
        </span>
      </button>
    </EventPopover>
  );
}

/** "+2 more" — the day's full list, without leaving the month. */
function MoreLink({
  day,
  events,
  colors,
  hidden,
}: {
  day: Date;
  events: CalEvent[];
  colors: Map<number, string>;
  hidden: number;
}) {
  const [open, setOpen] = useState(false);
  const now = useNow();
  return (
    <Popover open={open} onOpenChange={setOpen}>
      <PopoverTrigger asChild>
        <button
          type="button"
          className="px-1 text-left text-[10px] text-muted-foreground hover:text-foreground"
        >
          +{hidden} more
        </button>
      </PopoverTrigger>
      <PopoverContent side="right" align="start" className="w-64 p-0">
        <p className="px-3 pt-2.5 pb-1.5 font-display text-[13px] font-semibold text-foreground">
          {day.toLocaleDateString("en-AU", { weekday: "long", day: "numeric", month: "short" })}
        </p>
        <div className="px-1 pb-2">
          {events.map((e) => (
            <EventPopover key={e.id} event={e} color={colors.get(e.subjectId) ?? ""}>
              <button
                type="button"
                className="flex w-full min-w-0 items-center gap-2 rounded-md px-2 py-1.5 text-left hover:bg-surface"
              >
                <EventMark
                  kind={e.kind}
                  color={
                    isPast(e, now)
                      ? "var(--color-chart-other)"
                      : (colors.get(e.subjectId) ?? "")
                  }
                  size={10}
                />
                <span className="min-w-0 flex-1">
                  <span className="block truncate text-[11.5px] text-foreground">
                    {e.title}
                  </span>
                  <span className="block text-[10.5px] text-muted-foreground">
                    {e.subjectCode} · {fmtEventTime(e)}
                  </span>
                </span>
              </button>
            </EventPopover>
          ))}
        </div>
      </PopoverContent>
    </Popover>
  );
}
