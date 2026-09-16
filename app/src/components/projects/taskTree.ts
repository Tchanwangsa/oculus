import type { DbProject, DbProjectTask, ProjectColumn } from "@/lib/projects";

/**
 * Shaping the flat task list the store hands out into what the three views
 * draw. Pure functions over rows that are already in memory — `getTasks`
 * returns a project's parents and subtasks in one query on purpose
 * (`app/src/lib/projects.ts`), so the grouping is the caller's job and belongs
 * in one place rather than in each view.
 */

/** A top-level task with its subtasks, in `position` order. */
export interface TaskNode {
  task: DbProjectTask;
  children: DbProjectTask[];
}

/**
 * Parents first, each carrying its own children.
 *
 * A subtask whose parent is not in the list — which the schema cannot produce,
 * but a half-applied agent write could — is promoted to a top-level row rather
 * than dropped, on the same reasoning as `toProject`'s column fallback: a row
 * nothing draws is a row nobody can fix.
 */
export function taskTree(tasks: DbProjectTask[]): TaskNode[] {
  const byId = new Map(tasks.map((t) => [t.id, t]));
  const nodes: TaskNode[] = [];
  const byParent = new Map<number, TaskNode>();

  for (const task of tasks) {
    if (task.parent_id != null && byId.has(task.parent_id)) continue;
    const node: TaskNode = { task, children: [] };
    nodes.push(node);
    byParent.set(task.id, node);
  }
  for (const task of tasks) {
    if (task.parent_id == null) continue;
    byParent.get(task.parent_id)?.children.push(task);
  }
  return nodes;
}

/** The nodes sitting in one board column, in `position` order. */
export function nodesIn(nodes: TaskNode[], columnId: string): TaskNode[] {
  return nodes.filter((n) => n.task.column_id === columnId);
}

export interface SubtaskProgress {
  done: number;
  total: number;
  /** 0–100, and 0 rather than NaN when there are no subtasks. */
  pct: number;
}

export function subtaskProgress(node: TaskNode): SubtaskProgress {
  const total = node.children.length;
  const done = node.children.filter((c) => c.done_at != null).length;
  return { done, total, pct: total ? Math.round((done / total) * 100) : 0 };
}

/** How far the whole board has got: finished top-level tasks over all of them. */
export function boardProgress(nodes: TaskNode[]): { done: number; total: number } {
  return {
    done: nodes.filter((n) => n.task.done_at != null).length,
    total: nodes.length,
  };
}

export function columnOf(project: DbProject, columnId: string): ProjectColumn | null {
  return project.columns.find((c) => c.id === columnId) ?? null;
}

/**
 * The board's columns, backlog first.
 *
 * The backlog used to have a list view of its own beside the board, with a
 * promote button per stub. It went when the board's drag started working:
 * dragging a stub out of the backlog into Todo is the gesture a kanban board
 * exists for, and a second screen for making the same move was a screen to
 * keep in step for no gain.
 *
 * Leftmost because that is the direction of travel — a card's life runs left
 * to right across the board — and because `DEFAULT_COLUMNS` already opens that
 * way; the reordering here only matters for a board whose columns have since
 * been rearranged.
 */
export function boardColumns(project: DbProject): ProjectColumn[] {
  const backlog = project.columns.filter((c) => c.kind === "backlog");
  const rest = project.columns.filter((c) => c.kind !== "backlog");
  return [...backlog, ...rest];
}

/** Where a backlog stub goes when it is committed to: the first column that is
 *  work rather than a plan. Deliberately not `boardColumns(project)[0]`, which
 *  is now the backlog itself. */
export function promotionTarget(project: DbProject): ProjectColumn | null {
  return (
    project.columns.find((c) => c.kind === "active") ??
    project.columns.find((c) => c.kind !== "backlog") ??
    null
  );
}

/**
 * The `beforeId` / `afterId` pair for dropping `draggedId` onto `targetId`
 * inside one column — the only two arguments `moveTask` takes besides the
 * column, since a position is a midpoint rather than an index.
 *
 * The dragged task is taken out of the list first: dropping a card one slot
 * down means "after the card that is currently below me", and counting it as
 * its own neighbour would land it back where it started.
 */
export function dropSlot(
  columnNodes: TaskNode[],
  draggedId: number | null,
  targetId: number | null,
): { before: number | null; after: number | null } {
  const ids = columnNodes.map((n) => n.task.id).filter((id) => id !== draggedId);
  if (targetId == null) return { before: ids[ids.length - 1] ?? null, after: null };
  const i = ids.indexOf(targetId);
  if (i < 0) return { before: ids[ids.length - 1] ?? null, after: null };
  return { before: i > 0 ? ids[i - 1] : null, after: ids[i] };
}

/** The tail of a column — what an appended task is dropped after. */
export function appendSlot(
  nodes: TaskNode[],
  columnId: string,
): { before: number | null; after: number | null } {
  const ids = nodesIn(nodes, columnId).map((n) => n.task.id);
  return { before: ids[ids.length - 1] ?? null, after: null };
}
