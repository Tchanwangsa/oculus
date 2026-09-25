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
import { useWindowEvent } from "@/hooks/useEvents";
import { fmtShortDate } from "@/lib/format";

/**
 * The calendar event a project is pinned to. `project.event_id` is a
 * `CalEvent.id` spanning three tables, so it is not a foreign key (a sync
 * re-inserts Canvas rows, which would cascade the pin away) and is resolved
 * live against `loadCalendar()`. A pin that no longer resolves says so and
 * offers to clear itself.
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

  // Load only when there is a pin to resolve or the picker is open.
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

  // Reload on calendar updates only once a list is held.
  const loaded = events != null;
  useWindowEvent(CALENDAR_UPDATED_EVENT, () => {
    if (!loaded) return;
    loadCalendar()
      .then(setEvents)
      .catch((e) => console.error("reload calendar failed", e));
  });

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
              {pinned.subjectCode} · {fmtShortDate(pinned.start)} · {fmtEventTime(pinned)}
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

/** `fmtEventTime` is the clock alone; this adds the day. */
/** Opens on this subject's upcoming events; search reaches every subject and
 *  the past. `kind: "task"` rows are project tasks themselves, so excluded. */
function EventPicker({
  project,
  events,
  onPick,
}: {
  project: DbProject;
  /** `null` while loading. */
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
      if (project.subject_id != null && e.subjectId !== project.subject_id) return false;
      return (e.end ?? e.start).getTime() >= now;
    });

    // Upcoming soonest-first, then past most-recent-first.
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
              {e.subjectCode} · {fmtShortDate(e.start)} · {fmtEventTime(e)}
            </span>
          </button>
        ))}
      </div>
    </>
  );
}

const MAX_ROWS = 40;
