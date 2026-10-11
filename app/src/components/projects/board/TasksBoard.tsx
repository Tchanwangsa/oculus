import { useMemo, useRef } from "react";
import { ArrowElbowDownRight } from "@phosphor-icons/react";
import { cn } from "@/lib/utils";
import { SubjectIcon } from "@/components/subjects/SubjectIcon";
import { useCardDrag, useSettledList } from "@/hooks/gestures/useCardDrag";
import { displayCode } from "@/lib/format/format";
import type { DbProject, DbTaskWithProject, ProjectColumn } from "@/lib/planning/projects";
import { BoardView } from "./BoardParts";
import { CardTitle } from "./CardTitle";
import { taskHref } from "../nav/taskHref";
import { AgentMark, DueChip, TaskGlyph } from "../tasks/TaskMarks";
import {
  columnForUniversal,
  projectLabel,
  universalColumnOf,
} from "../tasks/universalTasks";

/**
 * Every task on the default board (Backlog, Todo, In progress, Done). A card
 * sits in the column matching its own board's column id, else its kind; a drop
 * writes back onto the card's own board the same way, and a board with no such
 * column refuses the drop.
 *
 * There is no manual order across projects, so a column is *sorted*
 * (`UNIVERSAL_ORDER`, carried in from `getAllTasks` — nothing here re-sorts)
 * and a drag within one is a no-op; see `../tasks/universalTasks.ts`. Subtasks are
 * plain cards naming their parent, since the sort puts them anywhere.
 */
export function TasksBoard({
  tasks,
  columns,
  projectById,
  onMove,
}: {
  /** In `UNIVERSAL_ORDER`. */
  tasks: DbTaskWithProject[];
  /** Which of `UNIVERSAL_COLUMNS` to draw, in order (the page filters it). */
  columns: readonly ProjectColumn[];
  projectById: Map<number, DbProject>;
  /** Given a column id off the task's own board, never an invented one. */
  onMove: (task: DbTaskWithProject, columnId: string) => void;
}) {
  // The drop resolves against a ref: the grouping below needs the gesture's
  // state, which needs this callback.
  const tasksRef = useRef(tasks);
  tasksRef.current = tasks;

  const drag = useCardDrag(
    ({ id, from, containerId }) => {
      // No manual order: back in its own column, there is nothing to write.
      if (from === containerId) return;
      const task = tasksRef.current.find((t) => t.id === id);
      if (!task) return;
      const column = columnForUniversal(task, projectById, containerId);
      // No column of that id or kind on the task's own board: spring back.
      if (!column) return;
      // Two universal columns can resolve to the same column on the task's
      // own board (one active column takes Todo and In progress).
      if (column.id === task.column_id) return;
      onMove(task, column.id);
      return true;
    },
    { settleOn: tasks },
  );
  const live = drag.drag;

  // During a settle, draw the list the gesture was measured against — see
  // `CardDragState.settling`.
  const settledTasks = useSettledList(tasks, live);

  const byId = useMemo(
    () => new Map(settledTasks.map((t) => [t.id, t])),
    [settledTasks],
  );

  const byColumn = useMemo(() => {
    const grouped = new Map(columns.map((c) => [c.id, [] as DbTaskWithProject[]]));
    // A task whose universal column is filtered out has no bucket and is not drawn.
    for (const task of settledTasks) {
      grouped.get(universalColumnOf(task, projectById).id)?.push(task);
    }
    return grouped;
  }, [columns, projectById, settledTasks]);

  // Back over its own column a drop does nothing, so nothing slides apart.
  const idle = live != null && live.targetContainerId === live.containerId;

  const lifted = live ? byId.get(live.id) ?? null : null;

  return (
    <BoardView
      columns={columns}
      byColumn={byColumn}
      drag={drag}
      lifted={lifted}
      idOf={(task) => task.id}
      hrefOf={(task) => taskHref(task.project_id, task)}
      frozen={idle}
      renderCard={(task) => (
        <TaskCard
          task={task}
          parent={task.parent_id != null ? byId.get(task.parent_id) ?? null : null}
          projectById={projectById}
        />
      )}
    />
  );
}

/**
 * What is on a card — its own component because the card is drawn twice while
 * dragged. It leads with the subject and project, which a project's own board
 * does not need.
 */
function TaskCard({
  task,
  parent,
  projectById,
}: {
  task: DbTaskWithProject;
  /** `null` unless the parent is in the list; a subtask whose parent was
   *  filtered out reads as a plain task. */
  parent: DbTaskWithProject | null;
  projectById: Map<number, DbProject>;
}) {
  return (
    <>
      <div className="flex min-w-0 items-center gap-1.5 text-[11px] text-muted-foreground">
        {task.project_subject_code && (
          <SubjectIcon code={task.project_subject_code} size={11} />
        )}
        <span
          className={cn(
            "truncate",
            task.project_id == null && "text-muted-foreground/60",
          )}
        >
          {task.project_subject_code
            ? `${displayCode(task.project_subject_code)} · ${projectLabel(task)}`
            : projectLabel(task)}
        </span>
      </div>

      {parent && (
        <div className="mt-0.5 flex min-w-0 items-center gap-1 text-[11px] text-muted-foreground/60">
          <ArrowElbowDownRight size={10} className="shrink-0" />
          <span className="truncate">{parent.title}</span>
        </div>
      )}

      <div className="mt-1 flex min-w-0 items-start gap-1.5">
        <TaskGlyph
          kind={universalColumnOf(task, projectById).kind}
          size={13}
          className="mt-0.5"
        />
        <CardTitle
          title={task.title}
          href={taskHref(task.project_id, task)}
          done={task.done_at != null}
        />
        <AgentMark source={task.source} className="mt-0.5" />
      </div>

      {task.due_at && (
        <div className="mt-1.5 pl-[18px]">
          <DueChip dueAt={task.due_at} />
        </div>
      )}
    </>
  );
}
