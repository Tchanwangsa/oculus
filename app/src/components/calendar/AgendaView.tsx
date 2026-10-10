import { useNow } from "@/hooks/ui/useNow";
import type { CalEvent } from "@/lib/planning/calendar";
import { sameDay, startOfDay } from "@/lib/format/format";
import { EventRow } from "./EventRow";
import { ListCard } from "@/components/ui/layout/PageParts";

/** Everything ahead, grouped by day. Past events are left to month and week. */
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
            <ListCard>
              {items.map((e) => (
                <EventRow
                  key={e.id}
                  event={e}
                  color={colors.get(e.subjectId) ?? ""}
                  now={now}
                />
              ))}
            </ListCard>
          </section>
        ))}
      </div>
    </div>
  );
}
