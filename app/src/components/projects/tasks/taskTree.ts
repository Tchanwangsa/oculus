import { boardOf, type DbProject, type DbProjectTask, type ProjectColumn } from "@/lib/planning/projects";

/**
 * Pure shaping of the flat task list (`getTasks` returns parents and subtasks
 * in one query) into what the views draw.
 */

/** A top-level task with its subtasks, in `position` order. */
export interface TaskNode<T extends DbProjectTask = DbProjectTask> {
  task: T;
  children: T[];
}

/**
 * Parents first, each carrying its own children. A subtask whose parent is
 * missing (a half-applied agent write) is promoted to top level, not dropped:
 * a row nothing draws is a row nobody can fix.
 */
export function taskTree<T extends DbProjectTask>(tasks: T[]): TaskNode<T>[] {
  const byId = new Map(tasks.map((t) => [t.id, t]));
  const nodes: TaskNode<T>[] = [];
  const byParent = new Map<number, TaskNode<T>>();

  for (const task of tasks) {
    if (task.parent_id != null && byId.has(task.parent_id)) continue;
    const node: TaskNode<T> = { task, children: [] };
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
function nodesIn(nodes: TaskNode[], columnId: string): TaskNode[] {
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

/**
 * The column a task names. A `null` project (unfiled task) uses `boardOf`'s
 * default board; a column the board no longer has resolves to `null`.
 */
export function columnOf(
  project: DbProject | null,
  columnId: string,
): ProjectColumn | null {
  return boardOf(project).find((c) => c.id === columnId) ?? null;
}

/** The board's columns, backlog first (a card's life runs left to right). */
export function boardColumns(project: DbProject): ProjectColumn[] {
  const backlog = project.columns.filter((c) => c.kind === "backlog");
  const rest = project.columns.filter((c) => c.kind !== "backlog");
  return [...backlog, ...rest];
}

/** Where a backlog stub goes when committed to: the first active column.
 *  Not `boardColumns(project)[0]`, which is the backlog itself. */
export function promotionTarget(project: DbProject | null): ProjectColumn | null {
  const board = boardOf(project);
  return (
    board.find((c) => c.kind === "active") ??
    board.find((c) => c.kind !== "backlog") ??
    null
  );
}

/** The tail of a column — what an appended task is dropped after. */
export function appendSlot(
  nodes: TaskNode[],
  columnId: string,
): { before: number | null; after: number | null } {
  const ids = nodesIn(nodes, columnId).map((n) => n.task.id);
  return { before: ids[ids.length - 1] ?? null, after: null };
}
