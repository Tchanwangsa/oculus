import { useCallback, useMemo, useState } from "react";
import { Link } from "react-router-dom";
import { CaretRight } from "@phosphor-icons/react";
import { EventRow } from "@/components/calendar/EventRow";
import {
  CALENDAR_UPDATED_EVENT,
  loadCalendar,
  subjectColors,
  type CalEvent,
} from "@/lib/planning/calendar";
import { sameDay, startOfDay } from "@/lib/format/format";
import { PROJECTS_UPDATED_EVENT } from "@/lib/planning/projects";
import { useHomeSection } from "./useHomeSection";

/**
 * Calendar rows change on sync, and tasks (live off `project_tasks`) on any
 * project write. Module-level for a stable reference — see `useHomeSection`.
 */
const EVENTS = [CALENDAR_UPDATED_EVENT, PROJECTS_UPDATED_EVENT];

/** Today plus this many days ahead. */
const DAYS_AHEAD = 7;

/** Rows across all days; the calendar has the rest. */
const MAX_ROWS = 5;

/**
 * The next few calendar rows from the start of today, within the week, under
 * day labels; See more opens the calendar. `now` comes from the page: one
 * clock for all rows. The card stays when the week is empty.
 */
export function UpcomingCard({ now }: { now: Date }) {
  const [events, setEvents] = useState<CalEvent[] | null>(null);

  const reload = useCallback(() => {
    loadCalendar()
      .then(setEvents)
      .catch((e) => {
        console.error(e);
        setEvents([]);
      });
  }, []);

  useHomeSection(reload, EVENTS);

  // Colours keyed off the whole set, matching the calendar page.
  const colors = useMemo(() => subjectColors(events ?? []), [events]);

  const week = useMemo(() => {
    const from = startOfDay(now);
    // Calendar days, not 24 h steps, so a DST change can't drop the last day.
    const to = new Date(from.getFullYear(), from.getMonth(), from.getDate() + DAYS_AHEAD + 1);
    return (events ?? []).filter((e) => e.start >= from && e.start < to);
  }, [events, now]);

  const shown = week.slice(0, MAX_ROWS);

  // `loadCalendar` sorts by start, so each day is a contiguous run.
  const groups: { day: Date; items: CalEvent[] }[] = [];
  for (const e of shown) {
    const last = groups[groups.length - 1];
    if (last && sameDay(last.day, e.start)) last.items.push(e);
    else groups.push({ day: e.start, items: [e] });
  }

  return (
    <section className="overflow-hidden rounded-lg border border-border">
      <div className="flex items-center justify-between px-3 pt-3">
        <h2 className="text-[13px] font-semibold text-foreground">Upcoming</h2>
        <Link
          to="/calendar"
          className="flex items-center gap-0.5 text-[11px] text-muted-foreground transition-colors hover:text-foreground"
        >
          See more
          <CaretRight size={10} />
        </Link>
      </div>
      {events && groups.length === 0 && (
        <p className="px-3 pt-2 pb-3 text-[12px] text-muted-foreground">Nothing in the next week</p>
      )}
      {groups.map((g) => (
        <div key={g.day.toDateString()}>
          <p className="px-3 pt-3 pb-1 text-[11px] font-medium text-muted-foreground">
            {dayLabel(g.day, now)}
          </p>
          <div className="divide-y divide-border-subtle">
            {g.items.map((e) => (
              <EventRow key={e.id} event={e} color={colors.get(e.subjectId) ?? ""} now={now} />
            ))}
          </div>
        </div>
      ))}
    </section>
  );
}

/** "Today", "Tomorrow", else "Thursday 8 Oct". */
function dayLabel(day: Date, now: Date): string {
  if (sameDay(day, now)) return "Today";
  if (sameDay(day, new Date(now.getFullYear(), now.getMonth(), now.getDate() + 1))) return "Tomorrow";
  return day.toLocaleDateString("en-AU", { weekday: "long", day: "numeric", month: "short" });
}
