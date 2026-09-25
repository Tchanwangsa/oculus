import { boardOf, type DbProject, type DbProjectTask, type ProjectColumn } from "@/lib/projects";

/**
 * Pure shaping of the flat task list (`getTasks` returns parents and subtasks
 * in one query) into what the views draw.
 */

/** A top-level task with its subtasks, in `position` order. */
export interface TaskNode {
  task: DbProjectTask;
  children: DbProjectTask[];
}

/**
 * Parents first, each carrying its own children. A subtask whose parent is
 * missing (a half-applied agent write) is promoted to top level, not dropped:
 * a row nothing draws is a row nobody can fix.
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
function nodesIn(nodes: TaskNode[], columnId: string): TaskNode[] {
  return nodes.filter((n) => n.task.column_id === columnId);
}

/** One card on the board. */
export interface ColumnEntry {
  task: DbProjectTask;
  /** The schema allows one level of nesting. */
  depth: 0 | 1;
  /** The parent of a `depth: 1` entry, `null` at depth 0. */
  parent: DbProjectTask | null;
  /** A subtask in this column whose parent sits in another one — normal, and
   *  the card names its parent so it doesn't read as homeless. */
  orphaned: boolean;
  /** For a parent card's progress meter; `null` at depth 1. */
  node: TaskNode | null;
}

/**
 * One column's cards in drawing order: each top-level task followed by its
 * children that are also here, then the orphaned subtasks last. The grouping
 * is structural: a subtask's `position` only orders it among its siblings, so
 * it can never sit between unrelated cards (see {@link siblingDropSlot}).
 */
export function columnEntries(nodes: TaskNode[], columnId: string): ColumnEntry[] {
  const entries: ColumnEntry[] = [];
  for (const node of nodes) {
    if (node.task.column_id !== columnId) continue;
    entries.push({ task: node.task, depth: 0, parent: null, orphaned: false, node });
    for (const child of node.children) {
      if (child.column_id !== columnId) continue;
      entries.push({ task: child, depth: 1, parent: node.task, orphaned: false, node: null });
    }
  }
  // Orphans go at the end of the column.
  for (const node of nodes) {
    if (node.task.column_id === columnId) continue;
    for (const child of node.children) {
      if (child.column_id !== columnId) continue;
      entries.push({ task: child, depth: 1, parent: node.task, orphaned: true, node: null });
    }
  }
  return entries;
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

/** The subtask row with this id, or `null` if it is top-level or absent. */
function subtaskById(nodes: TaskNode[], id: number): DbProjectTask | null {
  for (const node of nodes) {
    const hit = node.children.find((c) => c.id === id);
    if (hit) return hit;
  }
  return null;
}

/**
 * The `before`/`after` pair for `moveTask` when dropping `taskId` at `index`
 * in the column's flat visual list (dragged card removed, as `useCardDrag`
 * reports it). That list interleaves parents and subtasks, and a midpoint
 * against a non-sibling would be re-sorted away by {@link columnEntries} — so
 * the slot only counts how many of the card's own siblings lie above it, and
 * the pair comes from the siblings. No siblings here: append to the column.
 */
export function siblingDropSlot(
  nodes: TaskNode[],
  columnId: string,
  taskId: number,
  index: number,
): { before: number | null; after: number | null } {
  const rest = columnEntries(nodes, columnId).filter((e) => e.task.id !== taskId);
  const slot = Math.max(0, Math.min(index, rest.length));

  // Level comes from the tree, not `parent_id`: `taskTree` promotes a subtask
  // with a missing parent to top level.
  const isTop = nodes.some((n) => n.task.id === taskId);
  const parentId = isTop ? null : subtaskById(nodes, taskId)?.parent_id ?? null;
  const isSibling = (e: ColumnEntry) =>
    isTop ? e.depth === 0 : e.depth === 1 && e.task.parent_id === parentId;

  const siblings: number[] = [];
  let above = 0;
  rest.forEach((e, i) => {
    if (!isSibling(e)) return;
    if (i < slot) above++;
    siblings.push(e.task.id);
  });

  if (siblings.length === 0) {
    return { before: rest[rest.length - 1]?.task.id ?? null, after: null };
  }
  return {
    before: above > 0 ? siblings[above - 1] : null,
    after: above < siblings.length ? siblings[above] : null,
  };
}

/** The tail of a column — what an appended task is dropped after. */
export function appendSlot(
  nodes: TaskNode[],
  columnId: string,
): { before: number | null; after: number | null } {
  const ids = nodesIn(nodes, columnId).map((n) => n.task.id);
  return { before: ids[ids.length - 1] ?? null, after: null };
}
