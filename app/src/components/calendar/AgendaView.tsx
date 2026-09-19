import { useNow } from "@/hooks/useNow";
import { sameDay, startOfDay, type CalEvent } from "@/lib/calendar";
import { EventRow } from "./EventRow";

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
            <h2 className="mb-2 px-0.5 text-[13px] font-semibold text-foreground">
              {day.toLocaleDateString("en-AU", {
                weekday: "long",
                day: "numeric",
                month: "short",
              })}
              {sameDay(day, now) && " · Today"}
            </h2>
            <div className="overflow-hidden rounded-lg border border-border divide-y divide-border-subtle">
              {items.map((e) => (
                <EventRow
                  key={e.id}
                  event={e}
                  color={colors.get(e.subjectId) ?? ""}
                  now={now}
                />
              ))}
            </div>
          </section>
        ))}
      </div>
    </div>
  );
}
