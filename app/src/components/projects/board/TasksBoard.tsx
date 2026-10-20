import { useMemo, useRef } from "react";
import { useCardDrag, useSettledList } from "@/hooks/gestures/useCardDrag";
import type { DbProject, DbTaskTag, DbTaskWithProject, ProjectColumn } from "@/lib/planning/projects";
import type { PropertyId } from "../display/settings";
import { InlineAdd } from "../fields/InlineAdd";
import { subtaskProgress, taskTree } from "../tasks/taskTree";
import { TaskCard } from "./TaskCard";
import { BoardView } from "./BoardParts";
import { taskHref } from "../nav/taskHref";
import {
  columnForUniversal,
  universalColumnOf,
} from "../tasks/universalTasks";

/**
 * Task cards on the default board or one project's own columns. A card
 * sits in the column matching its own board's column id, else its kind; a drop
 * writes back onto the card's own board the same way, and a board with no such
 * column refuses the drop.
 *
 * There is no manual order across projects, so a column is *sorted*
 * (the page's Display sort, carried in — nothing here re-sorts)
 * and a drag within one is a no-op; see `../tasks/universalTasks.ts`. Subtasks are
 * plain cards naming their parent, since the sort puts them anywhere.
 */
export function TasksBoard({
  tasks,
  columns,
  projectById,
  tagById,
  onMove,
  allTasks = tasks,
  project,
  properties = ["project", "subject", "tags", "due"],
  onCreate,
}: {
  /** In the Display sort's order. */
  tasks: DbTaskWithProject[];
  allTasks?: DbTaskWithProject[];
  project?: DbProject;
  properties?: readonly PropertyId[];
  onCreate?: (input: { title: string; columnId: string }) => void;
  /** The scoped board's columns in drawing order (the page filters them). */
  columns: readonly ProjectColumn[];
  projectById: Map<number, DbProject>;
  /** The tag registry by id, for the chips on a card. */
  tagById: Map<number, DbTaskTag>;
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
      const column = project ? project.columns.find((c) => c.id === containerId) : columnForUniversal(task, projectById, containerId);
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
      const id = project ? task.column_id : universalColumnOf(task, projectById).id;
      grouped.get(id)?.push(task);
    }
    return grouped;
  }, [columns, projectById, settledTasks, project]);

  // Back over its own column a drop does nothing, so nothing slides apart.
  const idle = live != null && live.targetContainerId === live.containerId;

  const lifted = live ? byId.get(live.id) ?? null : null;
  const allById = useMemo(() => new Map(allTasks.map((t) => [t.id, t])), [allTasks]);
  const progressById = useMemo(() => new Map(taskTree(allTasks).map((node) => [node.task.id, subtaskProgress(node)])), [allTasks]);
  const now = useMemo(() => new Date(), [tasks]);

  return (
    <BoardView
      columns={columns}
      byColumn={byColumn}
      drag={drag}
      lifted={lifted}
      idOf={(task) => task.id}
      hrefOf={(task) => taskHref(task.project_id, task)}
      frozen={idle}
      footer={onCreate ? (column) => <InlineAdd label="New task" placeholder="Task title" onAdd={(title) => onCreate({ title, columnId: column.id })} /> : undefined}
      renderCard={(task) => (
        <TaskCard
          task={task}
          parent={task.parent_id != null ? allById.get(task.parent_id) ?? null : null}
          progress={progressById.get(task.id)}
          projectById={projectById}
          tagById={tagById}
          properties={properties}
          now={now}
        />
      )}
    />
  );
}
