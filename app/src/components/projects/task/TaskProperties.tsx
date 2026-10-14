import { Flag, FolderSimple, CalendarBlank } from "@phosphor-icons/react";
import { StatusPill } from "@/components/projects/board/StatusPill";
import { DateTimeField } from "@/components/projects/fields/DateTimeField";
import { PILL } from "@/components/projects/fields/pill";
import { ProjectPicker } from "@/components/projects/nav/ProjectPicker";
import type { DbProject, DbProjectTask } from "@/lib/planning/projects";
import { cn } from "@/lib/utils";
import { ChecklistProgress } from "./ChecklistProgress";
import { EstimatePill } from "./EstimatePill";
import { TaskTags } from "./TaskTags";

/**
 * The task's properties as one wrapping row of pills, each edited in place:
 * status, project, due, start, estimate, tags, then the checklist's progress.
 * Status writes through `onMove` (only `moveTask` keeps `done_at` in step with
 * the column); everything else goes through `onPatch`.
 */
export function TaskProperties({
  task,
  project,
  projects,
  checklist,
  onMove,
  onRefile,
  onPatch,
}: {
  task: DbProjectTask;
  /** `null` on an unfiled task. */
  project: DbProject | null;
  projects: DbProject[];
  /** The subtasks; empty for a subtask, which has no checklist. */
  checklist: DbProjectTask[];
  onMove: (columnId: string) => void;
  onRefile: (projectId: number | null) => void;
  onPatch: (patch: {
    dueAt?: string | null;
    startsAt?: string | null;
    estimateMinutes?: number | null;
    tagIds?: number[];
  }) => void;
}) {
  const projectName = project?.name ?? "No project";

  return (
    <div className="mt-4 flex flex-wrap items-center gap-2">
      <StatusPill
        project={project}
        columnId={task.column_id}
        onPick={onMove}
        dot
        className="h-7 px-2.5 text-xs"
      />

      {/* `refileTask` refuses to move a subtask on its own, so for one this is
          a readout, not a picker. */}
      {task.parent_id != null ? (
        <span
          title="A subtask follows its parent's project"
          className="inline-flex h-7 max-w-full min-w-0 items-center gap-1.5 rounded-full border border-border-subtle px-2.5 text-xs text-muted-foreground"
        >
          <FolderSimple size={13} className="shrink-0" />
          <span className="truncate">{projectName}</span>
          <span className="shrink-0 text-[11px] text-muted-foreground/70">follows parent</span>
        </span>
      ) : (
        <ProjectPicker
          projects={projects}
          value={task.project_id}
          label="Move to"
          unfiledHint="Take it out of every project"
          onPick={onRefile}
        >
          <button
            type="button"
            aria-label={`Project: ${projectName}`}
            className={cn(PILL, project ? "text-foreground" : "text-muted-foreground")}
          >
            <FolderSimple size={13} className="shrink-0 text-muted-foreground" />
            <span className="truncate">{projectName}</span>
          </button>
        </ProjectPicker>
      )}

      <DateTimeField
        variant="pill"
        icon={Flag}
        label="Due date"
        placeholder="Due date"
        value={task.due_at}
        defaultTime="end"
        onCommit={(dueAt) => onPatch({ dueAt })}
      />
      <DateTimeField
        variant="pill"
        icon={CalendarBlank}
        label="Start date"
        placeholder="Start"
        value={task.starts_at}
        defaultTime="start"
        onCommit={(startsAt) => onPatch({ startsAt })}
      />

      <EstimatePill
        minutes={task.estimate_minutes}
        onCommit={(estimateMinutes) => onPatch({ estimateMinutes })}
      />

      <TaskTags task={task} onChange={(tagIds) => onPatch({ tagIds })} />

      <ChecklistProgress items={checklist} />
    </div>
  );
}
