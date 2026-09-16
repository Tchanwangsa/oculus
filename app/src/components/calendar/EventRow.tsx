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
 * One event as a list row, shared by the calendar's Agenda view and Home's
 * Today list — which is why the two read identically.
 *
 * `now` is a prop, not a `useNow()` call in here: the caller owns the clock, so
 * a list of six rows runs one minute timer between them rather than six.
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
  // Today's list still carries the classes you have already sat
  // through — greyed, so "what's left today" reads at a glance.
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
            {/* A task names the project it belongs to: which
                piece of work this is part of is what tells you
                what to do about it. */}
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
