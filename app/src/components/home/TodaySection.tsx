import { useCallback, useMemo, useState } from "react";
import { Link } from "react-router-dom";
import { EventRow } from "@/components/calendar/EventRow";
import {
  CALENDAR_UPDATED_EVENT,
  loadCalendar,
  sameDay,
  startOfDay,
  subjectColors,
  type CalEvent,
} from "@/lib/calendar";
import { PROJECTS_UPDATED_EVENT } from "@/lib/projects";
import { ROW, Section } from "./Section";
import { useHomeSection } from "./useHomeSection";

/**
 * The calendar's documented contract, and both halves of it are needed here: a
 * sync raises CALENDAR_UPDATED_EVENT for the Canvas rows it replaced, and every
 * project write raises PROJECTS_UPDATED_EVENT — tasks are calendar events too,
 * read live off `project_tasks`, so one ticked off on its board has to leave
 * this list without a refresh.
 *
 * Module-level so the reference is stable — see `useHomeSection`.
 */
const EVENTS = [CALENDAR_UPDATED_EVENT, PROJECTS_UPDATED_EVENT];

/** Six rows is about a full teaching day. Past that the list stops being a
 *  glance and the calendar is one click away, which is what the tail row is
 *  for. */
const MAX_ROWS = 6;

/**
 * What is next, as the calendar's own rows.
 *
 * It shows the **first day that has anything**, not literally today: a
 * Saturday with no classes would otherwise be a heading called "Today" over an
 * empty box, or — under the no-placeholder rule — nothing at all, on a page
 * whose whole job is to answer "what's next". So the heading says Today when
 * it is today and names the day when it isn't, and scanning a week remains the
 * calendar's job rather than something Home half-does.
 *
 * `now` comes from the page. One clock for the heading, the past-event greying
 * and every row in the list — six `useNow()`s would be six minute timers
 * re-rendering out of step with each other.
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

  // Keyed off the whole set rather than the day being drawn, so a subject
  // keeps its colour here and on the calendar page.
  const colors = useMemo(() => subjectColors(events ?? []), [events]);

  const group = useMemo(() => {
    const from = startOfDay(now).getTime();
    const upcoming = (events ?? []).filter((e) => e.start.getTime() >= from);
    // `loadCalendar` returns them sorted, so the first day is the leading run
    // of same-day events — the Agenda's grouping, stopped after one group.
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
        // The overflow is a row in the same column, not a chip under it: it is
        // the seventh line of the list, and dressing it as a button would make
        // it the loudest thing in the section.
        <Link to="/calendar" className={`${ROW} text-[12px] text-muted-foreground`}>
          {rest} more
        </Link>
      )}
    </Section>
  );
}
