import type { EventItem, YoursItem } from "@/hooks/data/overview";
import { cn } from "@/lib/utils";
import { daysFrom, eventLabel, weekday } from "./format";
import { eventGlyph, yoursGlyph } from "./glyphs";
import type { OverviewOpener } from "./open";
import { MiniRow, Quiet, RowDate, RowTitle, SectionHeading } from "./rows";

/** Events (the week's classes, past ones grey) and what the student made that week. */
export function SideColumn({ events, yours, opener, now, thisWeek }: {
  events: EventItem[];
  yours: YoursItem[];
  opener: OverviewOpener;
  now: Date;
  /** The selection is the current week, so "this week" reads true. */
  thisWeek: boolean;
}) {
  return (
    <div className="min-w-0 space-y-6">
      <section>
        <SectionHeading>Events</SectionHeading>
        {events.length === 0 ? <Quiet>{thisWeek ? "No classes this week." : "No classes that week."}</Quiet> : events.map((item) => {
          const { Icon, tone } = eventGlyph(item);
          const href = item.href;
          const today = !item.past && daysFrom(now, item.date) === 0;
          return (
            <MiniRow
              key={item.id}
              muted={item.past}
              onOpen={href ? () => opener.open(href) : undefined}
              tabHref={href ? opener.tabHref(href) : undefined}
              title={item.location ?? undefined}
            >
              <Icon size={14} className={cn("shrink-0", item.past ? "text-muted-foreground/60" : tone)} />
              <RowTitle>{item.title}</RowTitle>
              <RowDate className={cn(item.past && "text-muted-foreground/60", today && "text-brand")}>
                {eventLabel(item.date, item.allDay, now)}
              </RowDate>
            </MiniRow>
          );
        })}
      </section>
      <section>
        <SectionHeading aside={thisWeek ? "this week" : undefined}>Yours</SectionHeading>
        {yours.length === 0 ? <Quiet>Nothing yet.</Quiet> : yours.map((item) => {
          const { Icon, tone } = yoursGlyph(item);
          return (
            <MiniRow key={item.id} onOpen={() => opener.open(item.href)} tabHref={opener.tabHref(item.href)}>
              <Icon size={14} className={cn("shrink-0", tone)} />
              <RowTitle>{item.title}</RowTitle>
              <RowDate>{weekday(item.date)}</RowDate>
            </MiniRow>
          );
        })}
      </section>
    </div>
  );
}
