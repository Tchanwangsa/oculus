import { CaretDown } from "@phosphor-icons/react";
import { cn } from "@/lib/utils";
import { fmtFullStamp, fmtRelativeDay, sqliteUtcToMs } from "@/lib/format/format";
import type { DbProject, DbTaskTag, DbTaskWithProject } from "@/lib/planning/projects";
import { ProjectPicker } from "../nav/ProjectPicker";
import { TagList } from "../tags/TagList";
import { TagPicker } from "../tags/TagPicker";
import { DueChip, TaskSubject } from "../tasks/TaskMarks";
import { projectLabel } from "../tasks/universalTasks";

/** The trigger every editable cell shares: quiet until hovered, a caret at
 *  the end. A `button`, so nothing inside it may be one. */
const CELL_TRIGGER =
  "flex w-full min-w-0 max-w-full cursor-pointer items-center gap-1 rounded-md px-1 py-0.5 text-left outline-none transition-colors hover:bg-accent focus-visible:ring-[3px] focus-visible:ring-ring/50";

const QUIET = "text-[11px] text-muted-foreground/50";

export interface CellActions {
  /** A column id from the task's own board. */
  onMove: (task: DbTaskWithProject, columnId: string) => void;
  /** `null` unfiles. Top-level tasks only. */
  onRefile: (task: DbTaskWithProject, projectId: number | null) => void;
  onTags: (task: DbTaskWithProject, tagIds: number[]) => void;
  onCreateTag: (name: string) => Promise<DbTaskTag>;
}

export function TagsCell({
  task,
  tags,
  tagById,
  onTags,
  onCreateTag,
}: {
  task: DbTaskWithProject;
  tags: DbTaskTag[];
  tagById: Map<number, DbTaskTag>;
} & Pick<CellActions, "onTags" | "onCreateTag">) {
  return (
    <TagPicker
      tags={tags}
      value={task.tag_ids}
      onChange={(ids) => onTags(task, ids)}
      onCreate={onCreateTag}
    >
      <button type="button" aria-label="Change tags" className={CELL_TRIGGER}>
        {task.tag_ids.length > 0 ? (
          <TagList tagIds={task.tag_ids} byId={tagById} max={2} />
        ) : (
          <span className={QUIET}>—</span>
        )}
        <CaretDown size={9} weight="bold" className="ml-auto shrink-0 text-muted-foreground/50" />
      </button>
    </TagPicker>
  );
}

export function ProjectCell({
  task,
  projects,
  onRefile,
}: {
  task: DbTaskWithProject;
  projects: DbProject[];
} & Pick<CellActions, "onRefile">) {
  // Gated on `parent_id`, not on the parent being in this list:
  // `refileTask` refuses any subtask.
  if (task.parent_id != null) {
    return (
      <span
        className={cn(
          "truncate px-1 text-[11px]",
          task.project_id == null ? "text-muted-foreground/60" : "text-foreground",
        )}
        title={`${projectLabel(task)} — a subtask sits in its parent's project`}
      >
        {projectLabel(task)}
      </span>
    );
  }
  return (
    <ProjectPicker
      projects={projects}
      value={task.project_id}
      label="Move to"
      unfiledHint="Take it out of every project"
      onPick={(projectId) => onRefile(task, projectId)}
    >
      <button
        type="button"
        aria-label="Change project"
        title={`${projectLabel(task)} — click to refile`}
        className={CELL_TRIGGER}
      >
        <span
          className={cn(
            "min-w-0 truncate text-[11px]",
            task.project_id == null ? "text-muted-foreground/60" : "text-foreground",
          )}
        >
          {projectLabel(task)}
        </span>
        <CaretDown size={9} weight="bold" className="ml-auto shrink-0 text-muted-foreground/50" />
      </button>
    </ProjectPicker>
  );
}

export function SubjectCell({ task }: { task: DbTaskWithProject }) {
  return (
    <span className="flex min-w-0 px-1">
      <TaskSubject code={task.project_subject_code} unfiled={task.project_id == null} />
    </span>
  );
}

export function DueCell({ task }: { task: DbTaskWithProject }) {
  return (
    <span className="truncate px-1">
      {task.due_at ? <DueChip dueAt={task.due_at} /> : <span className={QUIET}>—</span>}
    </span>
  );
}

/** A SQLite timestamp as "Today", "Yesterday", a weekday or a date; the whole
 *  stamp is its tooltip. */
export function StampCell({ value, now }: { value: string; now: Date }) {
  const ms = sqliteUtcToMs(value);
  if (ms == null) return <span className={cn(QUIET, "px-1")}>—</span>;
  return (
    <span
      title={fmtFullStamp(ms)}
      className="truncate px-1 text-[11px] tabular-nums text-muted-foreground"
    >
      {fmtRelativeDay(ms, now)}
    </span>
  );
}
