import {
  boardOf,
  DEFAULT_COLUMNS,
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
 * What a drag across columns *does* write is a change of column on the task's
 * own board: see {@link UNIVERSAL_COLUMNS} and {@link columnForUniversal}.
 */

/**
 * The universal board's columns: **literally {@link DEFAULT_COLUMNS}**, the
 * board every project is born with.
 *
 * An alias rather than a second list of the same four names, because two lists
 * of the same four names is how the two drift. A project renames its columns
 * and may add its own, so no cross-project view can use a project's vocabulary
 * — but the default board's *is* the shared one, since every project starts
 * there and an unfiled task's `column_id` is one of these four ids by
 * construction (`boardOf`).
 *
 * They are matched **id first, kind as fallback** — see
 * {@link universalColumnOf}. That is what keeps Todo and In progress apart
 * here, which a kind alone cannot do: they are both `active`.
 */
export const UNIVERSAL_COLUMNS: readonly ProjectColumn[] = DEFAULT_COLUMNS;

/**
 * Where a kind goes when an id cannot be matched.
 *
 * `active` lands on **In progress**, not Todo: a board that renamed or
 * replaced its active columns has no way to claim Todo specifically, and In
 * progress is the kind's plain name — the word `DEFAULT_COLUMNS` gives the
 * kind when it is not being split in two.
 */
const HOME_OF_KIND: Record<ColumnKind, string> = {
  backlog: "backlog",
  active: "doing",
  done: "done",
};

/** The project a task is filed in, or `null` when it is filed nowhere — which
 *  `boardOf` and `columnOf` both take as "the default board". */
export function projectOf(
  task: DbTaskWithProject,
  projectById: Map<number, DbProject>,
): DbProject | null {
  return task.project_id != null ? projectById.get(task.project_id) ?? null : null;
}

/**
 * Which of {@link UNIVERSAL_COLUMNS} a task is drawn in.
 *
 * **Id first**: a task whose own board still uses one of the four default ids
 * — which is every task on an unedited board, and every unfiled task — draws
 * in that same column, so Todo and In progress stay two columns here exactly
 * as they are on the project's own board.
 *
 * **Kind second**, through {@link HOME_OF_KIND}, for a column a project added
 * or renamed the id of. That column means something every board shares; which
 * of the two `active` columns it would have been is not knowable, so it takes
 * the kind's home.
 *
 * A column its own board no longer has resolves to nothing at all, and those
 * fall to Backlog — the leftmost column, where a card's life starts — rather
 * than being dropped from the board, because a card nothing draws is a card
 * nobody can fix. It is not drawn as though it were filed correctly: the
 * table's `StatusPill` paints an unknown column `destructive` and names it,
 * which is the readout that says what actually happened.
 */
export function universalColumnOf(
  task: DbTaskWithProject,
  projectById: Map<number, DbProject>,
): ProjectColumn {
  const backlog = UNIVERSAL_COLUMNS[0];
  const own = columnOf(projectOf(task, projectById), task.column_id);
  if (!own) return backlog;
  return (
    UNIVERSAL_COLUMNS.find((c) => c.id === own.id) ??
    UNIVERSAL_COLUMNS.find((c) => c.id === HOME_OF_KIND[own.kind]) ??
    backlog
  );
}

/**
 * The column a drop onto universal column `universalId` means **on this task's
 * own board**.
 *
 * The same id if the board has it, so a default board receives the drop
 * exactly where it was aimed. Otherwise the first column of that id's *kind*:
 * a board that renamed `todo` to "Next" still takes a drop onto Todo, and
 * entering a kind puts you at its start rather than skipping to the end of it.
 *
 * `null` when the board has no column of that kind either — a board with no
 * Done column has nowhere for a drop onto Done to go, and refusing is the only
 * honest answer.
 */
export function columnForUniversal(
  task: DbTaskWithProject,
  projectById: Map<number, DbProject>,
  universalId: string,
): ProjectColumn | null {
  const board = boardOf(projectOf(task, projectById));
  const exact = board.find((c) => c.id === universalId);
  if (exact) return exact;
  const kind = UNIVERSAL_COLUMNS.find((c) => c.id === universalId)?.kind;
  if (!kind) return null;
  return board.find((c) => c.kind === kind) ?? null;
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
