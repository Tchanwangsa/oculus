import { cn } from "@/lib/utils";
import { isPast, minutesFromMidnight, type CalEvent } from "@/lib/planning/calendar";
import { EventMark } from "@/components/calendar/EventMark";
import { EventPopover } from "@/components/calendar/EventPopover";
import {
  HOUR_PX,
  MARKER_INSET,
  MARKER_PX,
  tintPct,
} from "@/components/calendar/week/constants";

/** An instant (deadline, pinned note) at its own time on the grid, over the classes. */
export function InstantMarker({
  event: e,
  fromHour,
  gridHeight,
  today,
  colors,
}: {
  event: CalEvent;
  fromHour: number;
  gridHeight: number;
  today: Date;
  colors: Map<number, string>;
}) {
  const gone = isPast(e, today);
  const tone = gone
    ? "var(--color-chart-other)"
    : (colors.get(e.subjectId) ?? "");
  // Centred on the minute, then pulled back inside the grid
  // (11:59pm would hang half off) — by at most half its height,
  // so it stays on the right hour.
  const exact =
    ((minutesFromMidnight(e.start) - fromHour * 60) / 60) * HOUR_PX;
  const top = Math.min(
    Math.max(exact - MARKER_PX / 2, MARKER_INSET),
    Math.max(
      MARKER_INSET,
      gridHeight - MARKER_PX - MARKER_INSET,
    ),
  );
  return (
    <EventPopover event={e} color={colors.get(e.subjectId) ?? ""}>
      <button
        type="button"
        className="absolute z-20 flex min-w-0 items-center gap-1 overflow-hidden rounded-[3px] border-l-2 px-1 text-left shadow-xs transition-colors hover:brightness-95 dark:hover:brightness-125"
        style={{
          top,
          height: MARKER_PX,
          left: 2,
          right: 2,
          borderLeftColor: tone,
          backgroundColor: `color-mix(in srgb, ${tone} ${tintPct(
            e,
            gone,
            38,
          )}%, var(--color-card))`,
        }}
      >
        <EventMark kind={e.kind} color={tone} size={9} />
        <span
          className={cn(
            "truncate text-[10px] font-medium leading-4",
            gone ? "text-muted-foreground" : "text-foreground",
          )}
        >
          {e.title}
        </span>
      </button>
    </EventPopover>
  );
}
