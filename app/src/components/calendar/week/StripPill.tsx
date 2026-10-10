import { cn } from "@/lib/utils";
import { fmtEventTime, isPast, type CalEvent } from "@/lib/planning/calendar";
import { EventMark } from "@/components/calendar/EventMark";
import { EventPopover } from "@/components/calendar/EventPopover";
import { tintPct } from "@/components/calendar/week/constants";

/** A deadline, note or task in the strip above the grid. */
export function StripPill({
  event: e,
  today,
  colors,
}: {
  event: CalEvent;
  today: Date;
  colors: Map<number, string>;
}) {
  const gone = isPast(e, today);
  const tone = gone ? "var(--color-chart-other)" : (colors.get(e.subjectId) ?? "");
  return (
    <EventPopover event={e} color={colors.get(e.subjectId) ?? ""}>
      <button
        type="button"
        className="flex w-full min-w-0 items-center gap-1 rounded-[4px] border-l-2 px-1.5 py-0.5 text-left transition-colors hover:brightness-95 dark:hover:brightness-125"
        style={{
          borderLeftColor: tone,
          backgroundColor: `color-mix(in srgb, ${tone} ${tintPct(
            e,
            gone,
            20,
          )}%, var(--color-card))`,
        }}
      >
        <EventMark kind={e.kind} color={tone} size={9} />
        {!e.allDay && (
          <span className="shrink-0 text-[9.5px] tabular-nums leading-4 text-muted-foreground">
            {fmtEventTime(e)}
          </span>
        )}
        <span
          className={cn(
            "truncate text-[10.5px] font-medium leading-4",
            gone ? "text-muted-foreground" : "text-foreground",
          )}
        >
          {e.title}
        </span>
      </button>
    </EventPopover>
  );
}
