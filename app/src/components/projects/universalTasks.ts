import {
  boardOf,
  type ColumnKind,
  type DbProject,
  type DbTaskWithProject,
  type ProjectColumn,
} from "@/lib/projects";
import { columnOf } from "./taskTree";

/**
 * The shaping a task view that spans every project needs, and the one rule
 * that makes it different from a project's own board.
 *
 * **There is no manual order in a universal column, and there cannot be.**
 * `position` is a fractional slot *inside one project's column* — the midpoint
 * between two neighbours in that one run of cards (`moveTask` in
 * `app/src/lib/projects.ts`). Two projects' positions are two unrelated number
 * lines, so ordering a column that mixes projects by `position` would be
 * arithmetic on unrelated units: 0.5 on one board is not before 1 on another
 * in any sense a user could predict, and a hand-set order would be scrambled
 * the next time either project renumbered its own column. So a universal
 * column is **sorted, not arranged**: by due date (nulls last), then project
 * (unfiled first), then `position` — which is exactly `UNIVERSAL_ORDER`, the
 * ORDER BY `getAllTasks` and `getUnfiledTasks` already come back in. A drag
 * within one column therefore has nothing to write and is a no-op
 * (`TasksBoard`), and the table's rows are sortable by header rather than
 * draggable (`TasksTable`).
 *
 * What a drag across columns *does* write is a change of **kind**, which every
 * board shares: see {@link KIND_COLUMNS} and {@link columnForKind}.
 */

/**
 * The universal board's three columns.
 *
 * Kinds, not columns: a project renames its columns and may have several of
 * one kind (a default board has Todo *and* In progress, both `active`), so the
 * only vocabulary every board shares is what a column *means*. The names are
 * `DEFAULT_COLUMNS`' own words for each kind, so the universal board and a new
 * project's board read the same.
 */
export const KIND_COLUMNS: ReadonlyArray<{ kind: ColumnKind; name: string }> = [
  { kind: "backlog", name: "Backlog" },
  { kind: "active", name: "In progress" },
  { kind: "done", name: "Done" },
];

/** The project a task is filed in, or `null` when it is filed nowhere — which
 *  `boardOf` and `columnOf` both take as "the default board". */
export function projectOf(
  task: DbTaskWithProject,
  projectById: Map<number, DbProject>,
): DbProject | null {
  return task.project_id != null ? projectById.get(task.project_id) ?? null : null;
}

/**
 * Which of the three columns a task is drawn in.
 *
 * A column its own board no longer has resolves to no kind at all, and those
 * fall to Backlog — the leftmost column, where a card's life starts — rather
 * than being dropped from the board, because a card nothing draws is a card
 * nobody can fix. It is not drawn as though it were filed correctly: the
 * table's `StatusPill` paints an unknown column `destructive` and names it,
 * which is the readout that says what actually happened.
 */
export function kindOf(
  task: DbTaskWithProject,
  projectById: Map<number, DbProject>,
): ColumnKind {
  return columnOf(projectOf(task, projectById), task.column_id)?.kind ?? "backlog";
}

/**
 * The column a drop into `kind` means **on this task's own board**.
 *
 * The first column of that kind: entering a kind puts you at its start, so a
 * card dragged out of Backlog lands in Todo rather than skipping to In
 * progress on a board that distinguishes them. That distinction is the
 * project's to make and survives every drag that does not change kind, because
 * such a drag writes nothing at all.
 *
 * `null` when the board has no column of that kind — a board with no Done
 * column has nowhere for a drop onto Done to go, and refusing is the only
 * honest answer.
 */
export function columnForKind(
  task: DbTaskWithProject,
  projectById: Map<number, DbProject>,
  kind: ColumnKind,
): ProjectColumn | null {
  return boardOf(projectOf(task, projectById)).find((c) => c.kind === kind) ?? null;
}

/**
 * What `moveTask`'s `beforeId` is for appending a task to the end of a column:
 * the last task **of its own project** sitting there.
 *
 * Its own project's, because that is the run of cards its `position` is
 * comparable with — `moveTask` orders it against `project_id IS <its own>`, so
 * a neighbour from another project would be a midpoint between two unrelated
 * numbers. With `afterId: null` this lands the task at `last + 1`, which is
 * the end of that column, which is where a view with no manual order can
 * honestly put it.
 */
export function appendNeighbour(
  tasks: DbTaskWithProject[],
  task: DbTaskWithProject,
  columnId: string,
): number | null {
  let tail: DbTaskWithProject | null = null;
  for (const t of tasks) {
    if (t.id === task.id) continue;
    if (t.project_id !== task.project_id) continue;
    if (t.column_id !== columnId) continue;
    if (!tail || t.position > tail.position) tail = t;
  }
  return tail?.id ?? null;
}

/** The label a project cell wears. A task filed nowhere reads as **Unfiled** —
 *  the word the rest of this feature uses for it — never as an empty cell or a
 *  dash, which say "no data" about a state that is perfectly definite. */
export function projectLabel(task: DbTaskWithProject): string {
  return task.project_name ?? "Unfiled";
}
