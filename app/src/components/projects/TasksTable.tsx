import { useMemo, useState } from "react";
import { Link } from "react-router-dom";
import { ArrowElbowDownRight, CaretDown, CaretUp } from "@phosphor-icons/react";
import { cn } from "@/lib/utils";
import { SubjectIcon } from "@/components/subjects/SubjectIcon";
import { TablePagination, usePagedRows } from "@/components/ui/TablePagination";
import { displayCode, sqliteUtcToMs } from "@/lib/format";
import type { DbProject, DbTaskWithProject } from "@/lib/projects";
import { ProjectPicker } from "./ProjectPicker";
import { StatusPill } from "./StatusPill";
import { taskHref } from "./taskHref";
import { AgentMark, DueChip, TaskGlyph } from "./TaskMarks";
import {
  UNIVERSAL_COLUMNS,
  projectLabel,
  projectOf,
  universalColumnOf,
} from "./universalTasks";

/**
 * Every task in the library as one flat table — the view for reading what you
 * have on rather than working one project's plan.
 *
 * Structurally `app/src/components/sync/SyncHistoryTable.tsx`, which is the
 * table grammar in this app: one `COLS` template shared by the header and
 * every row, the header *outside* the scroller with a `pr-1.5` gutter
 * standing in for the scrollbar's width, and a pinned `TablePagination`
 * footer.
 *
 * **Sortable by header, and deliberately not drag-reorderable** — the one way
 * it departs from `ProjectTable`, whose rows *do* reorder. `position` is only
 * comparable inside one project's column (`./universalTasks.ts`), so there is
 * no manual order across projects to drag a row into: a grip here could only
 * write a number that means nothing, or nothing at all. Sorting is what a
 * cross-project list has instead, and the default — Due ascending, empties
 * last, ties broken by project then position — is exactly `UNIVERSAL_ORDER`,
 * the order the rows arrive in.
 *
 * Subtasks are rows like anything else, marked with their parent rather than
 * nested under it: the sort has already put the two wherever their due dates
 * put them, so there is nothing to nest inside.
 *
 * **Two cells write.** Status is the `StatusPill` every view carries, and
 * Project is a `ProjectPicker` — this is the one screen that shows a task
 * beside every project it could belong to, so it is where filing one is a
 * pick rather than a decision taken at create time. A *subtask's* Project
 * cell is text: a subtask sits in its parent's project, and `refileTask`
 * refuses to move one on its own, so offering the pick would be offering a
 * refusal.
 */

/** Column template shared by the header and every row. The first cell is the
 *  row number alone — `ProjectTable` shares that 28px with a drag grip, and
 *  there is no drag here to share it with. */
const COLS =
  "grid grid-cols-[28px_minmax(0,1fr)_150px_110px_130px_120px] items-center gap-3 px-5";

const PAGE_SIZE = 25;

/** Which cell a click on a header sorts by. */
type SortKey = "title" | "project" | "subject" | "status" | "due";

interface Sort {
  key: SortKey;
  dir: "asc" | "desc";
}

const HEADERS: ReadonlyArray<{ key: SortKey | null; label: string }> = [
  { key: null, label: "" },
  { key: "title", label: "Title" },
  { key: "project", label: "Project" },
  { key: "subject", label: "Subject" },
  { key: "status", label: "Status" },
  { key: "due", label: "Due" },
];

/**
 * `UNIVERSAL_ORDER` in TypeScript: due date (empties last), then project
 * (unfiled first), then `position`, then id.
 *
 * It is the tiebreak under every sort, and — since its first term *is* the Due
 * comparator — sorting by Due ascending reproduces the order the rows came
 * back in. Kept in step with the ORDER BY in `app/src/lib/projects.ts` by
 * hand: the query cannot express the other five sorts and the table cannot
 * re-query for each one, so one of the two orders has to be written twice.
 */
function defaultCmp(a: DbTaskWithProject, b: DbTaskWithProject): number {
  const due = dueCmp(a, b);
  if (due !== 0) return due;
  // Unfiled first — the list you are expected to empty.
  if ((a.project_id == null) !== (b.project_id == null)) return a.project_id == null ? -1 : 1;
  if (a.project_id != null && b.project_id != null && a.project_id !== b.project_id) {
    return a.project_id - b.project_id;
  }
  return a.position - b.position || a.id - b.id;
}

/** Soonest first, undated last. Through `sqliteUtcToMs` rather than comparing
 *  the strings: `due_at` is an ISO stamp from the UI *or* SQLite's own
 *  "YYYY-MM-DD HH:MM:SS" from the CLI, and those two do not sort against each
 *  other lexicographically — the `T` beats the space and reorders a day. */
function dueCmp(a: DbTaskWithProject, b: DbTaskWithProject): number {
  const am = sqliteUtcToMs(a.due_at);
  const bm = sqliteUtcToMs(b.due_at);
  if (am == null || bm == null) return am == null ? (bm == null ? 0 : 1) : -1;
  return am - bm;
}

export function TasksTable({
  tasks,
  projects,
  projectById,
  empty,
  onMove,
  onRefile,
}: {
  tasks: DbTaskWithProject[];
  /** Every project, archived included — what the Project cell's picker
   *  offers. `projectById` is the same list keyed for `boardOf`. */
  projects: DbProject[];
  projectById: Map<number, DbProject>;
  /** What an empty list says. The page knows whether it is showing everything
   *  or only the unfiled, and those are two different empties. */
  empty: string;
  /** Given a column id off the task's own board, never an invented one. */
  onMove: (task: DbTaskWithProject, columnId: string) => void;
  /** `null` files it nowhere. Only ever called for a top-level task. */
  onRefile: (task: DbTaskWithProject, projectId: number | null) => void;
}) {
  const [sort, setSort] = useState<Sort>({ key: "due", dir: "asc" });

  const byId = useMemo(() => new Map(tasks.map((t) => [t.id, t])), [tasks]);

  const sorted = useMemo(() => {
    const keyCmp = (a: DbTaskWithProject, b: DbTaskWithProject): number => {
      switch (sort.key) {
        case "title":
          return a.title.localeCompare(b.title);
        // By what the cell says, so the order matches what is on screen —
        // which puts "Unfiled" among the project names rather than at an end.
        case "project":
          return projectLabel(a).localeCompare(projectLabel(b));
        // By the code, not the chip's text, so a subject's rows group and the
        // subject-less ones (Personal, and every unfiled task) sit together.
        case "subject":
          return (a.project_subject_code ?? "").localeCompare(b.project_subject_code ?? "");
        // By the universal column the row sits in, in the board's own
        // left-to-right order — never by the column's name, which is the
        // user's to change. It is the *same* rank the board draws the card at,
        // so Todo and In progress sort apart here exactly as they sit apart
        // there; ranking by kind instead would call them equal while the board
        // separates them, and the column would read as a broken sort.
        case "status":
          return (
            UNIVERSAL_COLUMNS.indexOf(universalColumnOf(a, projectById)) -
            UNIVERSAL_COLUMNS.indexOf(universalColumnOf(b, projectById))
          );
        case "due":
          return dueCmp(a, b);
      }
    };
    const rows = [...tasks];
    rows.sort((a, b) => {
      // An undated row stays at the bottom in both directions: a Due column
      // whose descending half opens with a block of blanks is a column you
      // have to scroll past to read. Every other key has no empty — an
      // unfiled task reads "Unfiled", a subject-less one sorts as blank —
      // so only this one is asked.
      if (sort.key === "due") {
        const am = a.due_at == null;
        const bm = b.due_at == null;
        if (am !== bm) return am ? 1 : -1;
      }
      const k = keyCmp(a, b);
      if (k !== 0) return sort.dir === "asc" ? k : -k;
      return defaultCmp(a, b);
    });
    return rows;
  }, [tasks, sort, projectById]);

  const { page, pageCount, setPage, pageRows } = usePagedRows(sorted, PAGE_SIZE);

  /** A header click sorts by that cell, ascending; clicking the active one
   *  turns it round. There is no third "unsorted" state to get back to —
   *  Due ascending *is* the unsorted order. */
  const pick = (key: SortKey) =>
    setSort((prev) =>
      prev.key === key
        ? { key, dir: prev.dir === "asc" ? "desc" : "asc" }
        : { key, dir: "asc" },
    );

  return (
    <div className="flex h-full flex-col">
      {/* The header sits OUTSIDE the scroll container, not sticky inside it:
          the scrollbar is a 6px classic bar that takes its gutter from the
          scroller's full height, so a header within it gets a bar drawn down
          its right edge. `pr-1.5` re-creates that gutter's width for the
          header and `scrollbar-gutter: stable` on the body keeps it reserved
          when there is nothing to scroll — without both, the header and its
          rows sit 6px out of column. */}
      <div className="shrink-0 pr-1.5">
        <div className={cn(COLS, "border-b border-border-subtle bg-card py-2")}>
          {HEADERS.map((h, i) =>
            h.key == null ? (
              <span key={i} />
            ) : (
              <button
                key={h.key}
                type="button"
                onClick={() => pick(h.key as SortKey)}
                className={cn(
                  "flex cursor-pointer items-center gap-1 text-left text-[11px] font-medium transition-colors",
                  sort.key === h.key
                    ? "text-foreground"
                    : "text-muted-foreground hover:text-foreground",
                )}
              >
                <span className="truncate">{h.label}</span>
                {sort.key === h.key &&
                  (sort.dir === "asc" ? (
                    <CaretUp size={9} weight="bold" className="shrink-0" />
                  ) : (
                    <CaretDown size={9} weight="bold" className="shrink-0" />
                  ))}
              </button>
            ),
          )}
        </div>
      </div>

      <div className="flex-1 min-h-0 overflow-y-auto [scrollbar-gutter:stable]">
        {tasks.length === 0 && (
          <p className="px-5 py-16 text-center text-xs text-muted-foreground">{empty}</p>
        )}

        <div className="divide-y divide-border-subtle">
          {pageRows.map((task, i) => {
            const project = projectOf(task, projectById);
            const parent =
              task.parent_id != null ? byId.get(task.parent_id) ?? null : null;
            return (
              <div
                key={task.id}
                /* No `group/row`: nothing in this row is revealed on hover.
                   `ProjectTable` needs it for the drag grip and the
                   add-subtask button, and this table has neither. */
                className={cn(COLS, "py-2 transition-colors hover:bg-surface/60")}
              >
                <span className="text-[11px] tabular-nums text-muted-foreground/60">
                  {(page - 1) * PAGE_SIZE + i + 1}
                </span>

                <div className="flex min-w-0 items-center gap-1.5">
                  <TaskGlyph kind={universalColumnOf(task, projectById).kind} />
                  {/* A subtask says so where it is, since the row it belongs
                      to is sorted wherever its own due date put it. The parent
                      names itself on hover rather than taking width in a cell
                      the title already needs. */}
                  {parent && (
                    <span
                      title={`Subtask of ${parent.title}`}
                      className="shrink-0 text-muted-foreground/60"
                    >
                      <ArrowElbowDownRight size={11} />
                    </span>
                  )}
                  <Link
                    to={taskHref(task.project_id, task)}
                    className={cn(
                      "truncate text-xs hover:underline",
                      task.done_at
                        ? "text-muted-foreground line-through"
                        : "text-foreground",
                    )}
                  >
                    {task.title}
                  </Link>
                  <AgentMark source={task.source} />
                </div>

                {/* Unfiled is a state, not missing data, so it is a word —
                    quieter than a project's name, and never a dash. */}
                {/* Gated on the row's own `parent_id`, not on whether the
                    parent was found in this list: a subtask whose parent is
                    missing from the rows on screen is still a subtask, and
                    `refileTask` would still refuse to move it alone. */}
                {task.parent_id != null ? (
                  <span
                    className={cn(
                      "truncate text-[11px]",
                      task.project_id == null
                        ? "text-muted-foreground/60"
                        : "text-foreground",
                    )}
                    title={`${projectLabel(task)} — a subtask sits in its parent's project`}
                  >
                    {projectLabel(task)}
                  </span>
                ) : (
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
                      className={cn(
                        "min-w-0 cursor-pointer truncate rounded-md px-1.5 py-0.5 text-left text-[11px] transition-colors hover:bg-accent",
                        task.project_id == null
                          ? "text-muted-foreground/60"
                          : "text-foreground",
                      )}
                    >
                      {projectLabel(task)}
                    </button>
                  </ProjectPicker>
                )}

                {task.project_id == null ? (
                  <span className="text-[11px] text-muted-foreground/50">—</span>
                ) : task.project_subject_code ? (
                  <span className="inline-flex min-w-0 items-center gap-1.5 rounded-full border border-border-subtle bg-surface px-2 py-0.5 text-[11px] text-foreground">
                    <SubjectIcon code={task.project_subject_code} size={11} />
                    <span className="truncate">
                      {displayCode(task.project_subject_code)}
                    </span>
                  </span>
                ) : (
                  <span className="text-[11px] text-muted-foreground/60">Personal</span>
                )}

                {/* The pill reads and writes the task's *own* board — the
                    default four for an unfiled task, which is what `boardOf`
                    hands back and what `moveTask` will check the pick
                    against. */}
                <StatusPill
                  project={project}
                  columnId={task.column_id}
                  onPick={(columnId) => onMove(task, columnId)}
                />

                {task.due_at ? (
                  <DueChip dueAt={task.due_at} />
                ) : (
                  <span className="text-[11px] text-muted-foreground/50">—</span>
                )}
              </div>
            );
          })}
        </div>
      </div>

      <TablePagination
        page={page}
        pageCount={pageCount}
        onPage={setPage}
        total={sorted.length}
        unit="task"
      />
    </div>
  );
}
