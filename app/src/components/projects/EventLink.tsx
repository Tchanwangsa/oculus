import { useEffect, useMemo, useState } from "react";
import { Link } from "react-router-dom";
import { CalendarBlank, LinkSimple, X } from "@phosphor-icons/react";
import { cn } from "@/lib/utils";
import { Input } from "@/components/ui/input";
import { Popover, PopoverContent, PopoverTrigger } from "@/components/ui/popover";
import {
  CALENDAR_UPDATED_EVENT,
  fmtEventTime,
  loadCalendar,
  type CalEvent,
} from "@/lib/calendar";
import type { DbProject } from "@/lib/projects";

/**
 * The calendar event a project answers to — the Canvas deadline an assignment
 * is actually submitted against.
 *
 * `project.event_id` holds a `CalEvent.id` exactly as `app/src/lib/calendar.ts`
 * mints it, so one column addresses three tables: a Canvas row is its Canvas
 * id, a local row is `local_<n>`, a lecture is its own. It is **not** a foreign
 * key and cannot be one — a sync deletes a subject's Canvas rows and
 * re-inserts them, so a cascade would clear every pin halfway through (see
 * migration 33). The pin is therefore resolved live here, against the same
 * `loadCalendar()` the calendar page draws from.
 *
 * Which means a pin can stop resolving — the assignment was unpublished, the
 * local row deleted — and that case says so and offers to clear itself.
 * Drawing nothing would leave the user with a link they set, cannot see, and
 * cannot get rid of.
 */
export function EventLink({
  project,
  onPick,
}: {
  project: DbProject;
  /** A `CalEvent.id`, or `null` to unpin. */
  onPick: (eventId: string | null) => void;
}) {
  const [open, setOpen] = useState(false);
  const [events, setEvents] = useState<CalEvent[] | null>(null);

  // `loadCalendar` reads four tables and returns a semester's worth of rows, so
  // it runs only when something on screen actually needs them: a pin to
  // resolve, or an open picker. A project with no pin and a closed popover
  // never touches the calendar at all.
  const needed = project.event_id != null || open;
  useEffect(() => {
    if (!needed || events) return;
    let cancelled = false;
    loadCalendar()
      .then((rows) => !cancelled && setEvents(rows))
      .catch((e) => {
        console.error("load calendar failed", e);
        if (!cancelled) setEvents([]);
      });
    return () => {
      cancelled = true;
    };
  }, [needed, events]);

  // Only once a list is already held: a sync that replaced the calendar's rows
  // has to re-resolve the pin, but it is not a reason to go and read a
  // calendar this row had decided it did not need.
  const loaded = events != null;
  useEffect(() => {
    if (!loaded) return;
    const onUpdated = () => {
      loadCalendar()
        .then(setEvents)
        .catch((e) => console.error("reload calendar failed", e));
    };
    window.addEventListener(CALENDAR_UPDATED_EVENT, onUpdated);
    return () => window.removeEventListener(CALENDAR_UPDATED_EVENT, onUpdated);
  }, [loaded]);

  const pinned = useMemo(
    () => events?.find((e) => e.id === project.event_id) ?? null,
    [events, project.event_id],
  );

  if (project.event_id != null) {
    if (!loaded) {
      return <span className="text-[11px] text-muted-foreground/60">Loading…</span>;
    }
    if (!pinned) {
      return (
        <span className="flex flex-wrap items-center gap-2">
          <span className="text-[11px] text-muted-foreground">
            The event this was pinned to is no longer on the calendar.
          </span>
          <UnpinButton onClick={() => onPick(null)} />
        </span>
      );
    }
    return (
      <span className="flex min-w-0 items-start gap-1.5">
        {/* Through to the calendar rather than to a detail card of its own:
            everything about the event — what else is due that week, what it
            clashes with — is on the grid, and this row is a pointer, not a
            copy. */}
        <Link
          to="/calendar"
          className="group/event flex min-w-0 flex-1 items-start gap-1.5"
        >
          <CalendarBlank size={12} className="mt-0.5 shrink-0 text-muted-foreground" />
          <span className="min-w-0">
            <span className="block truncate text-xs text-foreground group-hover/event:underline">
              {pinned.title}
            </span>
            <span className="block truncate text-[11px] text-muted-foreground">
              {pinned.subjectCode} · {fmtDay(pinned.start)} · {fmtEventTime(pinned)}
            </span>
          </span>
        </Link>
        <UnpinButton onClick={() => onPick(null)} />
      </span>
    );
  }

  return (
    <Popover open={open} onOpenChange={setOpen}>
      <PopoverTrigger asChild>
        <button
          type="button"
          className="inline-flex cursor-pointer items-center gap-1 rounded-full border border-border px-2 py-0.5 text-[11px] text-muted-foreground transition-colors hover:text-foreground"
        >
          <LinkSimple size={10} weight="bold" className="shrink-0" />
          Link an event
        </button>
      </PopoverTrigger>
      <PopoverContent align="start" className="w-80 p-2">
        <EventPicker
          project={project}
          events={events}
          onPick={(id) => {
            setOpen(false);
            onPick(id);
          }}
        />
      </PopoverContent>
    </Popover>
  );
}

function UnpinButton({ onClick }: { onClick: () => void }) {
  return (
    <button
      type="button"
      aria-label="Unpin this event"
      title="Unpin"
      onClick={onClick}
      className="mt-0.5 shrink-0 cursor-pointer text-muted-foreground transition-colors hover:text-foreground"
    >
      <X size={10} weight="bold" />
    </button>
  );
}

/** The day an event falls on. `fmtEventTime` is the clock alone — right on a
 *  grid whose column already says which day it is, and not enough on a row
 *  that is read entirely out of context. */
function fmtDay(d: Date): string {
  return d.toLocaleDateString("en-AU", { day: "numeric", month: "short" });
}

/**
 * Pick the event, out of everything on the calendar.
 *
 * The list opens on the case the feature exists for — this subject's next
 * deadline — and the search field reaches past it to every subject and back
 * into the past, because the default is a convenience rather than a rule: a
 * project written up after the fact is pinned to a deadline that has already
 * gone.
 *
 * `kind: "task"` rows are excluded outright. Those *are* project tasks read
 * live back onto the grid (see `docs/calendar.md`), so pinning a project to
 * one would point a project at its own work.
 */
function EventPicker({
  project,
  events,
  onPick,
}: {
  project: DbProject;
  /** `null` while the calendar is still being read. */
  events: CalEvent[] | null;
  onPick: (eventId: string) => void;
}) {
  const [query, setQuery] = useState("");

  const rows = useMemo(() => {
    if (!events) return [];
    const q = query.trim().toLowerCase();
    const now = Date.now();

    const candidates = events.filter((e) => {
      if (e.kind === "task") return false;
      if (q) return `${e.title} ${e.subjectCode}`.toLowerCase().includes(q);
      // Unsearched, the list is the one the user came for: this project's own
      // subject, and only what is still ahead. A personal project has no
      // subject to narrow by, so it gets everything upcoming instead.
      if (project.subject_id != null && e.subjectId !== project.subject_id) return false;
      return (e.end ?? e.start).getTime() >= now;
    });

    // Nearest first in both directions, future before past: an ascending sort
    // over a searched set would open on last year's Assignment 1.
    const upcoming = candidates
      .filter((e) => (e.end ?? e.start).getTime() >= now)
      .sort((a, b) => a.start.getTime() - b.start.getTime());
    const past = candidates
      .filter((e) => (e.end ?? e.start).getTime() < now)
      .sort((a, b) => b.start.getTime() - a.start.getTime());

    return [...upcoming, ...past].slice(0, MAX_ROWS);
  }, [events, query, project.subject_id]);

  return (
    <>
      <Input
        autoFocus
        value={query}
        placeholder="Search the calendar"
        onChange={(e) => setQuery(e.target.value)}
        className="h-8 rounded-lg text-[13px]"
      />

      <div className="-mx-1 mt-2 max-h-64 overflow-y-auto px-1">
        {events == null && (
          <p className="px-2 py-4 text-center text-[11px] text-muted-foreground">Loading…</p>
        )}

        {events != null && rows.length === 0 && (
          <p className="px-2 py-4 text-center text-[11px] text-muted-foreground">
            {query
              ? "Nothing on the calendar matches that."
              : "Nothing coming up for this subject. Search to reach past events and other subjects."}
          </p>
        )}

        {rows.map((e) => (
          <button
            key={e.id}
            type="button"
            onClick={() => onPick(e.id)}
            className={cn(
              "flex w-full cursor-pointer flex-col gap-0.5 rounded-md px-2 py-1.5 text-left transition-colors",
              "text-foreground hover:bg-accent",
            )}
          >
            <span className="min-w-0 truncate text-[12.5px]">{e.title}</span>
            <span className="min-w-0 truncate text-[11px] text-muted-foreground">
              {e.subjectCode} · {fmtDay(e.start)} · {fmtEventTime(e)}
            </span>
          </button>
        ))}
      </div>
    </>
  );
}

/** A semester across a handful of subjects is hundreds of rows; the picker is
 *  a place you find one you already have in mind, so the field does the
 *  narrowing and the list stays a list. */
const MAX_ROWS = 40;
