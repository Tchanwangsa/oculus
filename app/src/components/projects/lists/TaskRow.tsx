import type { CSSProperties } from "react";
import { Link } from "react-router-dom";
import { ArrowElbowDownRight } from "@phosphor-icons/react";
import { cn } from "@/lib/utils";
import type { DbProject, DbTaskTag, DbTaskWithProject } from "@/lib/planning/projects";
import { taskHref } from "../nav/taskHref";
import { StatusPill } from "../board/StatusPill";
import { SubtaskProgressIndicator } from "../tasks/SubtaskProgressIndicator";
import type { SubtaskProgress } from "../tasks/taskTree";
import { AgentMark, SubtaskToggle } from "../tasks/TaskMarks";
import { projectOf } from "../tasks/universalTasks";
import type { PropertyId } from "../display/settings";
import {
  DueCell,
  ProjectCell,
  StampCell,
  SubjectCell,
  TagsCell,
  type CellActions,
} from "./TaskCells";

/** One task as a table row: Name, then a cell per shown property. */
export function TaskRow({
  task,
  parent,
  nested = false,
  childrenCount = 0,
  childrenProgress,
  childrenOpen = true,
  onToggleChildren,
  properties,
  cols,
  colsStyle,
  now,
  projects,
  projectById,
  tags,
  tagById,
  actions,
}: {
  task: DbTaskWithProject;
  /** `null` unless the parent is in the list. */
  parent: DbTaskWithProject | null;
  nested?: boolean;
  childrenCount?: number;
  childrenProgress?: SubtaskProgress;
  childrenOpen?: boolean;
  onToggleChildren?: () => void;
  properties: readonly PropertyId[];
  cols: string;
  colsStyle: CSSProperties;
  now: Date;
  projects: DbProject[];
  projectById: Map<number, DbProject>;
  tags: DbTaskTag[];
  tagById: Map<number, DbTaskTag>;
  actions: CellActions;
}) {
  return (
    <div style={colsStyle} className={cn(cols, "py-1.5 transition-colors hover:bg-surface/60")}>
      <div className={cn("flex min-w-0 items-center gap-1.5", nested && "pl-6")}>
        {childrenCount > 0 && onToggleChildren && <SubtaskToggle expandable expanded={childrenOpen} onToggle={onToggleChildren} />}
        <StatusPill project={projectOf(task, projectById)} columnId={task.column_id} onPick={(id) => actions.onMove(task, id)} glyphOnly />
        {parent && (
          <span title={`Subtask of ${parent.title}`} className="shrink-0 text-muted-foreground/60">
            <ArrowElbowDownRight size={11} />
          </span>
        )}
        <Link
          to={taskHref(task.project_id, task)}
          className={cn(
            "truncate text-[13px] hover:underline",
            task.done_at ? "text-muted-foreground line-through" : "text-foreground",
          )}
        >
          {task.title}
        </Link>
        <AgentMark source={task.source} />
        {childrenProgress && childrenProgress.total > 0 && (
          childrenCount > 0 && onToggleChildren ? (
            <button
              type="button"
              aria-label={`${childrenProgress.done} of ${childrenProgress.total} subtasks complete; ${childrenOpen ? "collapse" : "expand"} subtasks`}
              aria-expanded={childrenOpen}
              onClick={onToggleChildren}
              className="ml-auto shrink-0 cursor-pointer rounded-full outline-none hover:bg-accent focus-visible:ring-[3px] focus-visible:ring-ring/50"
            >
              <SubtaskProgressIndicator progress={childrenProgress} />
            </button>
          ) : (
            <Link to={taskHref(task.project_id, task)} aria-label={`${childrenProgress.done} of ${childrenProgress.total} subtasks complete; open subtasks`} className="ml-auto shrink-0 rounded-full outline-none hover:bg-accent focus-visible:ring-[3px] focus-visible:ring-ring/50">
              <SubtaskProgressIndicator progress={childrenProgress} />
            </Link>
          )
        )}
      </div>

      {properties.map((id) => {
        switch (id) {
          case "tags":
            return (
              <TagsCell
                key={id}
                task={task}
                tags={tags}
                tagById={tagById}
                onTags={actions.onTags}
                onCreateTag={actions.onCreateTag}
              />
            );
          case "project":
            return <ProjectCell key={id} task={task} projects={projects} onRefile={actions.onRefile} />;
          case "subject":
            return <SubjectCell key={id} task={task} />;
          case "due":
            return <DueCell key={id} task={task} />;
          case "updated":
            return <StampCell key={id} value={task.updated_at} now={now} />;
          case "created":
            return <StampCell key={id} value={task.created_at} now={now} />;
        }
      })}
    </div>
  );
}
