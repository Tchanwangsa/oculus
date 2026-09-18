import { boardOf, type DbProject, type DbProjectTask, type ProjectColumn } from "@/lib/projects";

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

/** One card on the board: a task, how deep it is drawn, and the parent it
 *  hangs off when that parent is somewhere else. */
export interface ColumnEntry {
  task: DbProjectTask;
  /** 0 for a top-level task, 1 for a subtask. One level is all the schema
   *  allows (see `docs/projects.md`), so there is no deeper case to carry. */
  depth: 0 | 1;
  /** The parent of a `depth: 1` entry, `null` at depth 0. Carried rather than
   *  looked up again because the card draws its title when orphaned. */
  parent: DbProjectTask | null;
  /** A subtask sitting in this column whose parent sits in another one. Normal,
   *  not broken: a piece of work can be finished while the task it belongs to
   *  is still in progress, which `createTask`'s doc comment says outright. The
   *  card says whose it is, or it reads as a card with no home. */
  orphaned: boolean;
  /** The task with its children, for the progress meter a parent card draws.
   *  `null` at depth 1, which cannot have children of its own. */
  node: TaskNode | null;
}

/**
 * One column's cards, in the order the board draws them: each top-level task
 * in this column, immediately followed by whichever of its children are also
 * here, and then — last — the subtasks whose parent is in another column.
 *
 * **The grouping is structural, not positional, and that is the thing to hold
 * on to.** `position` orders the rows *within a column*, and a subtask's is
 * only ever compared against its own siblings': it says where among its
 * siblings the subtask goes, never where the group as a whole sits. Parentage
 * decides that. So a subtask cannot be ordered between two unrelated
 * top-level cards no matter what number it carries, and a drop that tried to
 * put it there would be undone the moment this function ran again — which is
 * exactly what {@link siblingDropSlot} exists to prevent.
 *
 * Order inside each run is the order `nodes` arrives in, which is `getTasks`'s
 * `ORDER BY position` carried through `taskTree` — the same trade
 * {@link nodesIn} makes.
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
  // The strays, after everything that has a parent to sit under here. A second
  // pass rather than a branch in the first, because they belong at the end of
  // the column and the first pass is walking it in drawing order.
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
 * The column a task names, off the board it is checked against.
 *
 * `project` may be `null`: an unfiled task has no project, and its board is
 * `boardOf`'s default one — the same board `moveTask` will check its next
 * column against. A column the board no longer has still resolves to `null`,
 * which is what a glyph or a pill draws as "filed somewhere this board cannot
 * show".
 */
export function columnOf(
  project: DbProject | null,
  columnId: string,
): ProjectColumn | null {
  return boardOf(project).find((c) => c.id === columnId) ?? null;
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
 *  is now the backlog itself. `null` — an unfiled task — is the default board,
 *  as everywhere else ({@link columnOf}). */
export function promotionTarget(project: DbProject | null): ProjectColumn | null {
  const board = boardOf(project);
  return (
    board.find((c) => c.kind === "active") ??
    board.find((c) => c.kind !== "backlog") ??
    null
  );
}

/** The subtask row with this id, or `null` if it is a top-level task (or not
 *  in the tree at all). */
function subtaskById(nodes: TaskNode[], id: number): DbProjectTask | null {
  for (const node of nodes) {
    const hit = node.children.find((c) => c.id === id);
    if (hit) return hit;
  }
  return null;
}

/**
 * The `beforeId` / `afterId` pair for dropping `taskId` into `columnId` at
 * `index` — the only two arguments `moveTask` takes besides the column, since
 * a position is a midpoint rather than an index.
 *
 * `index` is the slot in the column's **flat visual list** with the dragged
 * card taken out, which is what `useCardDrag` hands back: it hit-tests the
 * pointer against the rects it captured, and those are in DOM order. That
 * order interleaves parents and subtasks, so the neighbours either side of the
 * slot are frequently not the dragged card's own kin — and a midpoint taken
 * against a stranger is a number {@link columnEntries} will re-sort away the
 * next time it runs, leaving a drag that visibly does nothing.
 *
 * So the slot is only used to *count*: how many of the card's own siblings lie
 * above it. The pair then comes from that subset — the column's other
 * top-level cards for a top-level card, the same parent's other children here
 * for a subtask. A card with no siblings in this column has no pair to sit
 * between, so it goes after everything the column holds; its run is drawn
 * where parentage says it is either way.
 *
 * This supersedes the `dropSlot` the board used while it dropped *onto* a
 * target card, and keeps the part of it worth keeping: the dragged task is
 * taken out of the list first, because dropping a card one slot down means
 * "after the card that is currently below me", and counting it as its own
 * neighbour would take the midpoint of the gap it is already in and land it
 * back where it started.
 */
export function siblingDropSlot(
  nodes: TaskNode[],
  columnId: string,
  taskId: number,
  index: number,
): { before: number | null; after: number | null } {
  const rest = columnEntries(nodes, columnId).filter((e) => e.task.id !== taskId);
  const slot = Math.max(0, Math.min(index, rest.length));

  // Which level the card belongs to is read off the tree rather than off
  // `parent_id`: a subtask whose parent row is missing is promoted to top level
  // by `taskTree`, so it is drawn — and must be dropped — as a top-level card
  // even though it still names a parent.
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
