import { cn } from "@/lib/utils";
import { useNow } from "@/hooks/useNow";
import {
  fmtEventTime,
  isPast,
  sameDay,
  shortLocation,
  startOfDay,
  type CalEvent,
} from "@/lib/calendar";
import { EventMark } from "./EventMark";
import { EventPopover } from "./EventPopover";

const KIND_LABEL: Record<CalEvent["kind"], string> = {
  class: "class",
  due: "due",
  lecture: "recording",
  note: "note",
  task: "task",
};

/**
 * Everything ahead, in order, grouped by day — the view that answers "what's
 * next" without counting grid squares. Past events are left out: the month and
 * week views are where you go looking backwards.
 */
export function AgendaView({
  events,
  colors,
}: {
  events: CalEvent[];
  colors: Map<number, string>;
}) {
  const now = useNow();
  const from = startOfDay(now).getTime();
  const upcoming = events.filter((e) => e.start.getTime() >= from);

  if (upcoming.length === 0) {
    return (
      <div className="flex h-full items-center justify-center">
        <p className="text-xs text-muted-foreground">Nothing scheduled ahead.</p>
      </div>
    );
  }

  const days: { day: Date; items: CalEvent[] }[] = [];
  for (const e of upcoming) {
    const last = days[days.length - 1];
    if (last && sameDay(last.day, e.start)) last.items.push(e);
    else days.push({ day: e.start, items: [e] });
  }

  return (
    <div className="page-scroll">
      <div className="mx-auto max-w-3xl px-6 py-5 space-y-5">
        {days.map(({ day, items }) => (
          <section key={day.toDateString()}>
            <h2 className="mb-1.5 px-0.5 text-[11px] font-semibold uppercase tracking-wider text-muted-foreground">
              {day.toLocaleDateString("en-AU", {
                weekday: "long",
                day: "numeric",
                month: "short",
              })}
              {sameDay(day, now) && " · Today"}
            </h2>
            <div className="overflow-hidden rounded-lg border border-border divide-y divide-border-subtle">
              {items.map((e) => {
                const color = colors.get(e.subjectId) ?? "";
                // Today's list still carries the classes you have already sat
                // through — greyed, so "what's left today" reads at a glance.
                const gone = isPast(e, now);
                return (
                  <EventPopover key={e.id} event={e} color={color}>
                    <button
                      type="button"
                      className="flex w-full items-center gap-3 px-3 py-2.5 text-left transition-colors hover:bg-surface"
                    >
                      <EventMark
                        kind={e.kind}
                        color={gone ? "var(--color-chart-other)" : color}
                        size={12}
                      />
                      <span className="w-28 shrink-0 text-[11px] tabular-nums text-muted-foreground">
                        {fmtEventTime(e)}
                      </span>
                      <span className="min-w-0 flex-1">
                        <span
                          className={cn(
                            "block truncate text-[12px]",
                            gone ? "text-muted-foreground" : "text-foreground",
                          )}
                        >
                          {e.title}
                        </span>
                        <span className="block truncate text-[11px] text-muted-foreground">
                          {e.subjectCode}
                          {/* A task names the project it belongs to: which
                              piece of work this is part of is what tells you
                              what to do about it. */}
                          {e.kind === "task" && e.projectName ? (
                            ` · task · ${e.projectName}`
                          ) : e.kind === "due" ? (
                            <span className="font-medium text-foreground/70"> · due</span>
                          ) : (
                            ` · ${KIND_LABEL[e.kind]}`
                          )}
                          {e.location ? ` · ${shortLocation(e.location)}` : ""}
                        </span>
                      </span>
                    </button>
                  </EventPopover>
                );
              })}
            </div>
          </section>
        ))}
      </div>
    </div>
  );
}
