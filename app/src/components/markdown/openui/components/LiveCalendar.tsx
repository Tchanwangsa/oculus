import { Link } from "react-router-dom";
import { defineComponent } from "@openuidev/react-lang";
import { z } from "zod/v4";
import {
  loadCalendar,
  CALENDAR_UPDATED_EVENT,
  type CalEvent,
} from "@/lib/planning/calendar";
import { PROJECTS_UPDATED_EVENT } from "@/lib/planning/projects";
import { fmtFullStamp } from "@/lib/format/format";
import { Frame } from "./Frame";
import { useLiveData } from "./useLiveData";

const events = [CALENDAR_UPDATED_EVENT, PROJECTS_UPDATED_EVENT];
function EventRow({ event }: { event: CalEvent }) {
  return (
    <Link
      to={`/calendar?date=${event.start.toISOString().slice(0, 10)}`}
      className="flex flex-wrap items-baseline gap-2 py-2 text-sm hover:text-brand"
    >
      <span className="min-w-0 flex-1">{event.title}</span>
      <span className="text-xs text-muted-foreground">
        {event.subjectCode} · {fmtFullStamp(event.start.getTime())}
      </span>
    </Link>
  );
}
export const Deadline = defineComponent({
  name: "Deadline",
  description: "A live calendar event by its exact CLI event id.",
  props: z.object({ eventId: z.string() }),
  component: function DeadlineView({ props }) {
    const data = useLiveData("calendar", loadCalendar, events);
    const event = data.value?.find((e) => e.id === props.eventId);
    return (
      <Frame title="Deadline">
        {event ? (
          <EventRow event={event} />
        ) : (
          <p className="text-sm text-muted-foreground">
            {data.error ??
              (data.value
                ? "This event is no longer available."
                : "Loading deadline…")}
          </p>
        )}
      </Frame>
    );
  },
});
export const Upcoming = defineComponent({
  name: "Upcoming",
  description:
    "Live upcoming calendar items for a subject id or code; null shows all subjects. Days defaults to 7.",
  props: z.object({
    subject: z.union([z.number(), z.string()]).nullish(),
    days: z.number().int().min(1).max(365).nullish(),
  }),
  component: function UpcomingView({ props }) {
    const data = useLiveData("calendar", loadCalendar, events);
    const now = Date.now();
    const rows =
      data.value?.filter(
        (e) =>
          e.start.getTime() >= now &&
          e.start.getTime() <= now + (props.days ?? 7) * 86400000 &&
          (props.subject == null ||
            e.subjectId === props.subject ||
            e.subjectCode === props.subject),
      ) ?? [];
    return (
      <Frame title={`Upcoming · ${props.days ?? 7} days`}>
        <div className="divide-y divide-border-subtle">
          {rows.map((e) => (
            <EventRow key={e.id} event={e} />
          ))}
        </div>
        {!rows.length && (
          <p className="text-sm text-muted-foreground">
            {data.error ??
              (data.value
                ? "Nothing upcoming in this window."
                : "Loading calendar…")}
          </p>
        )}
      </Frame>
    );
  },
});
