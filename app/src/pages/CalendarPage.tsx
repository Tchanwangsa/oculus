import { useCallback, useEffect, useMemo, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { ArrowsClockwise, CaretLeft, CaretRight } from "@phosphor-icons/react";
import { Button } from "@/components/ui/button";
import { Skeleton } from "@/components/ui/skeleton";
import { cn } from "@/lib/utils";
import {
  getSubjects,
  replaceCalendarEvents,
  type CalendarEventData,
} from "@/lib/db";
import {
  CALENDAR_UPDATED_EVENT,
  addDays,
  addMonths,
  fmtMonth,
  fmtWeekRange,
  loadCalendar,
  startOfWeek,
  subjectColors,
  type CalEvent,
} from "@/lib/calendar";
import { MonthView } from "@/components/calendar/MonthView";
import { WeekView } from "@/components/calendar/WeekView";
import { AgendaView } from "@/components/calendar/AgendaView";

type View = "month" | "week" | "agenda";

const VIEWS: { id: View; label: string }[] = [
  { id: "month", label: "Month" },
  { id: "week", label: "Week" },
  { id: "agenda", label: "Upcoming" },
];

/**
 * Classes, deadlines and recordings across every subject, in one place.
 *
 * The rows come from Canvas's calendar API during a sync (see
 * `app/src-tauri/src/calendar.rs`); this page only reads them, and its refresh
 * button re-runs that fetch for the selected subjects without a full scrape.
 */
export default function CalendarPage() {
  const [view, setView] = useState<View>("week");
  const [anchor, setAnchor] = useState(() => new Date());
  const [events, setEvents] = useState<CalEvent[] | null>(null);
  const [hidden, setHidden] = useState<Set<number>>(new Set());
  const [refreshing, setRefreshing] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const reload = useCallback(() => {
    loadCalendar()
      .then(setEvents)
      .catch((e) => {
        console.error(e);
        setEvents([]);
      });
  }, []);

  useEffect(() => {
    reload();
    window.addEventListener(CALENDAR_UPDATED_EVENT, reload);
    return () => window.removeEventListener(CALENDAR_UPDATED_EVENT, reload);
  }, [reload]);

  // Colours are keyed off the full set, not the visible one, so hiding a
  // subject never recolours the others.
  const colors = useMemo(() => subjectColors(events ?? []), [events]);
  const subjects = useMemo(() => {
    const seen = new Map<number, string>();
    for (const e of events ?? []) seen.set(e.subjectId, e.subjectCode);
    return [...seen.entries()].sort((a, b) => a[1].localeCompare(b[1]));
  }, [events]);

  const shown = useMemo(
    () => (events ?? []).filter((e) => !hidden.has(e.subjectId)),
    [events, hidden],
  );

  /** Re-fetch the selected subjects' calendars without a full scrape. One
   *  subject failing (no calendar tab, an expired session) must not cost the
   *  rest, so failures are collected rather than thrown. */
  const refresh = () => {
    setRefreshing(true);
    setError(null);
    void (async () => {
      const failures: string[] = [];
      try {
        const selected = (await getSubjects()).filter((s) => s.selected);
        for (const s of selected) {
          try {
            const rows = await invoke<CalendarEventData[]>("calendar_sync_events", {
              canvasCourseId: s.id,
            });
            await replaceCalendarEvents(s.id, rows);
          } catch (e) {
            failures.push(`${s.code}: ${e}`);
          }
        }
      } catch (e) {
        failures.push(String(e));
      }
      if (failures.length > 0) setError(failures[0]);
      reload();
      setRefreshing(false);
    })();
  };

  const step = (dir: 1 | -1) =>
    setAnchor((a) =>
      view === "month" ? addMonths(a, dir) : addDays(startOfWeek(a), dir * 7),
    );

  const period =
    view === "month" ? fmtMonth(anchor) : view === "week" ? fmtWeekRange(anchor) : "";

  return (
    <div className="flex h-full flex-col overflow-hidden">
      <header className="shrink-0 border-b border-border-subtle px-6">
        <div className="flex items-center gap-3 pt-5 pb-3">
          <h1 className="text-[22px] font-semibold leading-none tracking-tight text-foreground">
            Calendar
          </h1>
          {period && (
            <span className="text-[13px] text-muted-foreground">{period}</span>
          )}

          <div className="flex-1" />

          {view !== "agenda" && (
            <div className="flex items-center gap-0.5">
              <Button variant="ghost" size="icon-sm" onClick={() => step(-1)} aria-label="Previous">
                <CaretLeft size={13} />
              </Button>
              <Button
                variant="ghost"
                size="sm"
                className="text-[12px]"
                onClick={() => setAnchor(new Date())}
              >
                Today
              </Button>
              <Button variant="ghost" size="icon-sm" onClick={() => step(1)} aria-label="Next">
                <CaretRight size={13} />
              </Button>
            </div>
          )}

          <div className="flex items-center rounded-md border border-border p-0.5">
            {VIEWS.map((v) => (
              <button
                key={v.id}
                type="button"
                onClick={() => setView(v.id)}
                className={cn(
                  "rounded-[4px] px-2 py-1 text-[11.5px] font-medium transition-colors",
                  view === v.id
                    ? "bg-surface-raised text-foreground"
                    : "text-muted-foreground hover:text-foreground",
                )}
              >
                {v.label}
              </button>
            ))}
          </div>

          <Button
            variant="ghost"
            size="icon-sm"
            onClick={refresh}
            disabled={refreshing}
            aria-label="Refresh calendar"
            className="text-muted-foreground/70 hover:text-foreground"
          >
            <ArrowsClockwise size={13} className={cn(refreshing && "animate-spin")} />
          </Button>
        </div>

        {subjects.length > 0 && (
          <div className="flex flex-wrap items-center gap-1.5 pb-2.5">
            {subjects.map(([id, code]) => {
              const off = hidden.has(id);
              return (
                <button
                  key={id}
                  type="button"
                  onClick={() =>
                    setHidden((h) => {
                      const next = new Set(h);
                      if (!next.delete(id)) next.add(id);
                      return next;
                    })
                  }
                  className={cn(
                    "inline-flex items-center gap-1.5 rounded-full border px-2 py-0.5 text-[11px] transition-colors",
                    off
                      ? "border-border text-muted-foreground/60"
                      : "border-border text-foreground hover:bg-surface",
                  )}
                >
                  <span
                    className="h-1.5 w-1.5 rounded-full"
                    style={{ backgroundColor: off ? "transparent" : colors.get(id) }}
                  />
                  {code}
                </button>
              );
            })}
          </div>
        )}

        {error && (
          <p className="pb-2 text-[11px] text-destructive">{error}</p>
        )}
      </header>

      <div className="min-h-0 flex-1">
        {events == null ? (
          <div className="space-y-2 px-6 py-6">
            {Array.from({ length: 5 }).map((_, i) => (
              <Skeleton key={i} className="h-12 w-full" />
            ))}
          </div>
        ) : events.length === 0 ? (
          <Empty refreshing={refreshing} onRefresh={refresh} />
        ) : view === "month" ? (
          <MonthView month={anchor} events={shown} colors={colors} />
        ) : view === "week" ? (
          <WeekView anchor={anchor} events={shown} colors={colors} />
        ) : (
          <AgendaView events={shown} colors={colors} />
        )}
      </div>
    </div>
  );
}

function Empty({
  refreshing,
  onRefresh,
}: {
  refreshing: boolean;
  onRefresh: () => void;
}) {
  return (
    <div className="flex h-full flex-col items-center justify-center gap-3 px-6 text-center">
      <p className="max-w-md text-sm text-muted-foreground">
        Nothing on the calendar yet. Class times and due dates come from Canvas
        during a sync — fetch them now without a full scrape.
      </p>
      <Button variant="secondary" size="sm" onClick={onRefresh} disabled={refreshing}>
        <ArrowsClockwise size={13} className={cn(refreshing && "animate-spin")} />
        Fetch from Canvas
      </Button>
      <p className="max-w-md text-[11px] text-muted-foreground/70">
        A subject only appears here if its staff publish events to the Canvas
        calendar. Where they don't, Oculus falls back to the Echo360 lecture
        recordings it has already synced.
      </p>
    </div>
  );
}
