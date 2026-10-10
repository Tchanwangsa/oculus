import { cn } from "@/lib/utils";
import {
  durationMinutes,
  fmtEventTime,
  isPast,
  minutesFromMidnight,
  shortLocation,
  type CalEvent,
} from "@/lib/planning/calendar";
import { EventPopover } from "@/components/calendar/EventPopover";
import { BLOCK_MIN_PX, HOUR_PX } from "@/components/calendar/week/constants";

/** A timed class, placed on its day's grid in its overlap lane. */
export function ClassBlock({
  event,
  lane,
  of,
  fromHour,
  gridHeight,
  today,
  colors,
}: {
  event: CalEvent;
  lane: number;
  of: number;
  fromHour: number;
  gridHeight: number;
  today: Date;
  colors: Map<number, string>;
}) {
  // Clamped into the grid like the markers: Canvas publishes
  // 11:59pm–11:59pm "classes" that would hang off the bottom.
  // A block with real length is shortened, not moved.
  const exact =
    ((minutesFromMidnight(event.start) - fromHour * 60) / 60) * HOUR_PX;
  const wanted = Math.max(
    BLOCK_MIN_PX,
    (durationMinutes(event) / 60) * HOUR_PX - 2,
  );
  const height = Math.max(
    BLOCK_MIN_PX,
    Math.min(wanted, gridHeight - exact),
  );
  const top = Math.max(0, Math.min(exact, gridHeight - height));
  const color = colors.get(event.subjectId) ?? "";
  // A finished class loses its subject colour.
  const gone = isPast(event, today);
  const tone = gone ? "var(--color-chart-other)" : color;
  return (
    <EventPopover event={event} color={color}>
      <button
        type="button"
        className="absolute overflow-hidden rounded-[4px] border-l-2 px-1.5 py-0.5 text-left transition-colors hover:brightness-95 dark:hover:brightness-125"
        style={{
          top,
          height,
          left: `calc(${(lane / of) * 100}% + 2px)`,
          width: `calc(${100 / of}% - 4px)`,
          borderLeftColor: tone,
          backgroundColor: `color-mix(in srgb, ${tone} ${gone ? 12 : 18}%, var(--color-card))`,
        }}
      >
        <span
          className={cn(
            "block truncate text-[10.5px] font-medium leading-4",
            gone ? "text-muted-foreground" : "text-foreground",
          )}
        >
          {event.title}
        </span>
        {height > 30 && (
          <span className="block truncate text-[10px] leading-3.5 text-muted-foreground">
            {fmtEventTime(event)}
            {event.location ? ` · ${shortLocation(event.location)}` : ""}
          </span>
        )}
      </button>
    </EventPopover>
  );
}
