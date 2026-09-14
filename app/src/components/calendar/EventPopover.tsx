import type { ReactNode } from "react";
import { Link } from "react-router-dom";
import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";
import { ArrowSquareOut, Kanban, MapPin, Play, Trash } from "@phosphor-icons/react";
import {
  Popover,
  PopoverContent,
  PopoverTrigger,
} from "@/components/ui/popover";
import { MD_COMPONENTS } from "@/components/markdown/MdComponents";
import {
  CALENDAR_UPDATED_EVENT,
  fmtEventTime,
  type CalEvent,
} from "@/lib/calendar";
import { deleteLocalEvent } from "@/lib/db";
import { projectHref } from "@/components/projects/projectHref";

const KIND_LABEL: Record<CalEvent["kind"], string> = {
  class: "Class",
  due: "Due",
  lecture: "Recording",
  note: "Note",
  task: "Task",
};

/** Where a local row came from, in the user's words. Only `manual` is written
 *  now; `automation` is kept for rows the removed automations feature left
 *  behind, which are still real events on the user's grid. */
const SOURCE_LABEL: Record<string, string> = {
  automation: "Added by an automation",
  manual: "Added by you",
};

/**
 * The detail card behind every chip in every view. Actions differ by layer: a
 * Canvas event or deadline links out to Canvas, a recording opens the in-app
 * player — nothing is shown for a link the event does not have.
 *
 * It is also where a local event is deleted. Canvas rows need no such control
 * (the next sync would write them straight back), so the affordance belongs
 * with the one layer that owns its own lifetime rather than in a panel of its
 * own. A task is not deletable here either, for a different reason: the
 * calendar only *reads* `project_tasks`, and everything you would do to a task
 * — re-date it, move its column, drop it — belongs on its board, which the card
 * links through to.
 */
export function EventPopover({
  event,
  color,
  children,
}: {
  event: CalEvent;
  color: string;
  children: ReactNode;
}) {
  const localId = event.localId;
  // A task's project, if this is one. The name travels in the href because the
  // tab strip titles a project tab from the query alone (`projectHref`).
  const project =
    event.projectId != null && event.projectName != null
      ? { id: event.projectId, name: event.projectName }
      : null;
  /** Nothing else ever clears a local event — no sync replaces the table — so
   *  removing one is final and immediate, like every other Remove in the app.
   *  The page reloads off the same signal a sync raises. */
  const remove = () => {
    if (localId == null) return;
    void deleteLocalEvent(localId)
      .then(() => window.dispatchEvent(new CustomEvent(CALENDAR_UPDATED_EVENT)))
      .catch(console.error);
  };

  return (
    <Popover>
      <PopoverTrigger asChild>{children}</PopoverTrigger>
      <PopoverContent side="right" align="start" className="w-80 p-0">
        <div className="px-3.5 pt-3 pb-2.5">
          <div className="flex items-center gap-1.5 mb-1">
            <span
              className="h-2 w-2 rounded-full shrink-0"
              style={{ backgroundColor: color }}
            />
            <span className="text-[11px] font-medium text-muted-foreground">
              {event.subjectCode}
            </span>
            <span className="text-[11px] text-muted-foreground/60">
              · {KIND_LABEL[event.kind]}
            </span>
          </div>

          <p className="text-[13px] font-medium text-foreground leading-snug">
            {event.title}
          </p>

          <p className="mt-1 text-[11px] text-muted-foreground">
            {event.start.toLocaleDateString("en-AU", {
              weekday: "long",
              day: "numeric",
              month: "long",
            })}
            {" · "}
            {fmtEventTime(event)}
          </p>

          {event.location && (
            <p className="mt-1 flex items-center gap-1 text-[11px] text-muted-foreground">
              <MapPin size={11} className="shrink-0" />
              {event.location}
            </p>
          )}
        </div>

        {event.description && (
          <div className="max-h-52 overflow-y-auto border-t border-border-subtle px-3.5 py-2.5 text-xs text-muted-foreground [&_p]:my-1 [&_p]:text-xs">
            <ReactMarkdown remarkPlugins={[remarkGfm]} components={MD_COMPONENTS}>
              {event.description}
            </ReactMarkdown>
          </div>
        )}

        {(event.url || event.lectureId || project || localId != null) && (
          <div className="border-t border-border-subtle px-3.5 py-2 flex items-center gap-3">
            {localId != null && (
              <>
                <span className="text-[11px] text-muted-foreground/70">
                  {SOURCE_LABEL[event.localSource ?? ""] ?? "Added in Oculus"}
                </span>
                <button
                  type="button"
                  onClick={remove}
                  className="ml-auto inline-flex items-center gap-1.5 text-[11px] text-muted-foreground hover:text-destructive"
                >
                  <Trash size={11} /> Remove
                </button>
              </>
            )}
            {project && (
              <Link
                to={projectHref(project)}
                className="inline-flex min-w-0 items-center gap-1.5 text-[11px] text-brand hover:underline"
              >
                <Kanban size={11} className="shrink-0" />
                <span className="truncate">{project.name}</span>
              </Link>
            )}
            {event.lectureId && (
              <Link
                to={`/subjects/${event.subjectId}/lecture?id=${encodeURIComponent(
                  event.lectureId,
                )}&t=${encodeURIComponent(event.title)}`}
                className="inline-flex items-center gap-1.5 text-[11px] text-brand hover:underline"
              >
                <Play size={11} weight="fill" /> Open recording
              </Link>
            )}
            {event.url && (
              <a
                href={event.url}
                target="_blank"
                rel="noreferrer"
                className="inline-flex items-center gap-1.5 text-[11px] text-brand hover:underline"
              >
                <ArrowSquareOut size={11} /> Open in Canvas
              </a>
            )}
          </div>
        )}
      </PopoverContent>
    </Popover>
  );
}
