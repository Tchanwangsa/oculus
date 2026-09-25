import { useState, type ReactNode } from "react";
import { Link } from "react-router-dom";
import {
  ArrowSquareOut,
  Kanban,
  MapPin,
  PencilSimple,
  Play,
  Trash,
} from "@phosphor-icons/react";
import {
  Popover,
  PopoverContent,
  PopoverTrigger,
} from "@/components/ui/popover";
import { CompactMd } from "@/components/markdown/MdComponents";
import {
  CALENDAR_UPDATED_EVENT,
  fmtEventTime,
  type CalEvent,
} from "@/lib/calendar";
import { deleteLocalEvent } from "@/lib/db";
import { editEvent } from "@/stores/eventEditorStore";
import { projectHref } from "@/components/projects/projectHref";

const KIND_LABEL: Record<CalEvent["kind"], string> = {
  class: "Class",
  due: "Due",
  lecture: "Recording",
  note: "Note",
  task: "Task",
};

/**
 * The detail card behind every chip in every view. Only local events get Edit
 * and Remove: a Canvas row would be written straight back by the next sync, and
 * a task is edited on its board, which the card links to.
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
  // Controlled so the card closes before the edit dialog opens: two stacked
  // focus traps leave the page unclickable.
  const [open, setOpen] = useState(false);
  const localId = event.localId;
  const project =
    event.projectId != null && event.projectName != null
      ? { id: event.projectId, name: event.projectName }
      : null;
  /** The page reloads off the same signal a sync raises. */
  const remove = () => {
    if (localId == null) return;
    void deleteLocalEvent(localId)
      .then(() => window.dispatchEvent(new CustomEvent(CALENDAR_UPDATED_EVENT)))
      .catch(console.error);
  };

  /** The dialog lives once in `AppLayout`; the `CalEvent` carries everything
   *  its form needs. */
  const edit = () => {
    setOpen(false);
    editEvent(event);
  };

  return (
    <Popover open={open} onOpenChange={setOpen}>
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
          <div className="max-h-52 overflow-y-auto border-t border-border-subtle px-3.5 py-2.5 text-muted-foreground">
            {/* `!` because `.md-compact` is unlayered and outranks utilities. */}
            <CompactMd text={event.description} className="text-xs! [&_p]:my-1!" />
          </div>
        )}

        {(event.url || event.lectureId || project || localId != null) && (
          <div className="border-t border-border-subtle px-3.5 py-2 flex items-center gap-3">
            {localId != null && (
              <>
                <span className="text-[11px] text-muted-foreground/70">
                  {event.localSource === "manual" ? "Added by you" : "Added in Oculus"}
                </span>
                <button
                  type="button"
                  onClick={edit}
                  className="ml-auto inline-flex items-center gap-1.5 text-[11px] text-muted-foreground hover:text-foreground"
                >
                  <PencilSimple size={11} /> Edit
                </button>
                <button
                  type="button"
                  onClick={remove}
                  className="inline-flex items-center gap-1.5 text-[11px] text-muted-foreground hover:text-destructive"
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
