import { CalendarBlank, Flag, User } from "@phosphor-icons/react";
import { SubjectIcon } from "@/components/subjects/SubjectIcon";
import { displayCode } from "@/lib/format/format";
import type { DbProject, UpdateProjectInput } from "@/lib/planning/projects";
import { cn } from "@/lib/utils";
import { DateTimeField } from "../fields/DateTimeField";
import { PILL } from "../fields/pill";
import { TagEditor } from "../fields/TagEditor";
import { ProgressMeter } from "../tasks/TaskMarks";
import { EventLink } from "./EventLink";

/** The same compact property rhythm as a task, with project-specific fields. */
export function ProjectProperties({ project, progress, onPatch }: {
  project: DbProject;
  progress: { done: number; total: number };
  onPatch: (patch: UpdateProjectInput) => void;
}) {
  return (
    <div className="mt-4 space-y-3" aria-label="Project properties">
      <div className="flex flex-wrap items-center gap-2">
        <span className={cn(PILL, "cursor-default bg-secondary text-foreground")}>
          <span className={cn("size-1.5 rounded-full", project.status === "active" ? "bg-brand" : "bg-muted-foreground")} />
          {project.status === "archived" ? "Archived" : "Active"}
        </span>
        <span className={cn(PILL, "cursor-default text-muted-foreground")} aria-label="Subject">
          {project.subject_code ? <><SubjectIcon code={project.subject_code} size={13} />{displayCode(project.subject_code)}</> : <><User size={13} />Personal</>}
        </span>
        <DateTimeField variant="pill" icon={Flag} label="Due date" placeholder="Due date"
          value={project.due_at} defaultTime="end" onCommit={(dueAt) => onPatch({ dueAt })} />
        <DateTimeField variant="pill" icon={CalendarBlank} label="Start date" placeholder="Start"
          value={project.starts_at} defaultTime="start" onCommit={(startsAt) => onPatch({ startsAt })} />
        {progress.total > 0 && <span className={cn(PILL, "cursor-default")} aria-label={`${progress.done} of ${progress.total} tasks complete`}>
          <ProgressMeter done={progress.done} total={progress.total} />
        </span>}
        <TagEditor tags={project.tags} onChange={(tags) => onPatch({ tags })} />
      </div>
      <EventLink project={project} onPick={(eventId) => onPatch({ eventId })} />
    </div>
  );
}
