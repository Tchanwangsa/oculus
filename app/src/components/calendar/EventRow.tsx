import { cn } from "@/lib/utils";
import {
  fmtEventTime,
  isPast,
  shortLocation,
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
 * One event as a list row, shared by Agenda and Home's Upcoming card. The caller
 * owns `now`, so a list runs one minute timer rather than one per row.
 */
export function EventRow({
  event,
  color,
  now,
}: {
  event: CalEvent;
  color: string;
  now: Date;
}) {
  // Classes already sat through are greyed, not dropped.
  const gone = isPast(event, now);
  return (
    <EventPopover event={event} color={color}>
      <button
        type="button"
        className="flex w-full items-center gap-3 px-3 py-2.5 text-left transition-colors hover:bg-surface"
      >
        <EventMark
          kind={event.kind}
          color={gone ? "var(--color-chart-other)" : color}
          size={12}
        />
        <span className="w-28 shrink-0 text-[11px] tabular-nums text-muted-foreground">
          {fmtEventTime(event)}
        </span>
        <span className="min-w-0 flex-1">
          <span
            className={cn(
              "block truncate text-[12px]",
              gone ? "text-muted-foreground" : "text-foreground",
            )}
          >
            {event.title}
          </span>
          <span className="block truncate text-[11px] text-muted-foreground">
            {event.subjectCode}
            {/* A task names its project. */}
            {event.kind === "task" && event.projectName ? (
              ` · task · ${event.projectName}`
            ) : event.kind === "due" ? (
              <span className="font-medium text-foreground/70"> · due</span>
            ) : (
              ` · ${KIND_LABEL[event.kind]}`
            )}
            {event.location ? ` · ${shortLocation(event.location)}` : ""}
          </span>
        </span>
      </button>
    </EventPopover>
  );
}
