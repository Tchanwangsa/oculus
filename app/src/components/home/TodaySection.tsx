import { useCallback, useMemo, useState } from "react";
import { Link } from "react-router-dom";
import { EventRow } from "@/components/calendar/EventRow";
import {
  CALENDAR_UPDATED_EVENT,
  loadCalendar,
  subjectColors,
  type CalEvent,
} from "@/lib/calendar";
import { sameDay, startOfDay } from "@/lib/format";
import { PROJECTS_UPDATED_EVENT } from "@/lib/projects";
import { ROW, Section } from "./Section";
import { useHomeSection } from "./useHomeSection";

/**
 * Calendar rows change on sync, and tasks (live off `project_tasks`) on any
 * project write. Module-level for a stable reference — see `useHomeSection`.
 */
const EVENTS = [CALENDAR_UPDATED_EVENT, PROJECTS_UPDATED_EVENT];

const MAX_ROWS = 6;

/**
 * The first day that has anything (not literally today), as calendar rows —
 * headed "Today" when it is. `now` comes from the page: one clock for all rows.
 */
export function TodaySection({ now }: { now: Date }) {
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

  const group = useMemo(() => {
    const from = startOfDay(now).getTime();
    const upcoming = (events ?? []).filter((e) => e.start.getTime() >= from);
  // Sorted, so the first day is the leading run of same-day events.
    const items: CalEvent[] = [];
    for (const e of upcoming) {
      if (items.length && !sameDay(items[0].start, e.start)) break;
      items.push(e);
    }
    return items.length ? { day: items[0].start, items } : null;
  }, [events, now]);

  if (!group) return null;

  const shown = group.items.slice(0, MAX_ROWS);
  const rest = group.items.length - shown.length;
  const title = sameDay(group.day, now)
    ? "Today"
    : group.day.toLocaleDateString("en-AU", {
        weekday: "long",
        day: "numeric",
        month: "short",
      });

  return (
    <Section title={title}>
      {shown.map((e) => (
        <EventRow key={e.id} event={e} color={colors.get(e.subjectId) ?? ""} now={now} />
      ))}
      {rest > 0 && (
        // Overflow is a quiet row in the column, not a button.
        <Link to="/calendar" className={`${ROW} text-[12px] text-muted-foreground`}>
          {rest} more
        </Link>
      )}
    </Section>
  );
}
