import { useCallback, useEffect, useMemo, useState } from "react";
import { PillTabs } from "@/components/ui/PillTabs";
import { NewTaskButton } from "@/components/projects/NewTaskButton";
import { SectionHeader } from "@/components/projects/SectionHeader";
import { TasksBoard } from "@/components/projects/TasksBoard";
import { TasksTable } from "@/components/projects/TasksTable";
import {
  DEFAULT_FILTER,
  TaskFilters,
  filteredColumns,
  isFiltered,
  matchesFilter,
  type TaskFilter,
} from "@/components/projects/TaskFilters";
import { appendNeighbour } from "@/components/projects/universalTasks";
import { useTaskList } from "@/hooks/useTaskList";
import type { DbTaskWithProject } from "@/lib/projects";
import { useProjectsStore } from "@/stores/projectsStore";

/**
 * Every task you have, across every project — and the ones that belong to no
 * project at all.
 *
 * A project's board is where one plan is arranged; this is the other question,
 * the one a board cannot answer: *what is there to do*. So it spans projects,
 * it opens on tasks rather than on plans, and it is where a task with nowhere
 * to go gets written down (`NewTaskButton` creates one unfiled by default —
 * migration 37, not an "Inbox" project).
 *
 * **What it gives up for spanning projects is manual order.** `position` is
 * only comparable inside one project's column, so nothing here can be
 * dragged into a slot: the board's columns are sorted (due date, then project,
 * then position) and a drag inside one is a no-op, and the table sorts by
 * header rather than by grip. `app/src/components/projects/universalTasks.ts`
 * is where that rule is written down and enforced.
 *
 * **The section's second tab**, under the `Projects · Tasks` strip the two
 * share (`SectionHeader`) — one sidebar row leads to both. Its own title went
 * with the strip's arrival: the strip names the section, and a heading on the
 * same rule would be a second title.
 *
 * **Four filters, opening on Todo**, which is the question the page exists to
 * answer. They replaced an `All tasks · Unfiled` strip: Unfiled is a *value* of
 * the project filter now, which also finally makes one project's tasks askable
 * for here. `app/src/components/projects/TaskFilters.tsx` holds their meaning;
 * this page holds their state and does the one pass. The status filter decides
 * which columns the board *has* — filter to Todo alone and there is one column
 * and nowhere to drag to, which is deliberate: a board that ignored a filter
 * the toolbar says is on would be worse. A status is changed from the table's
 * `StatusPill`, or by widening the filter.
 */
type TaskView = "board" | "table";

const VIEW_KEY = "oculus-tasks-view";

/**
 * The status set, and only the status set, survives a reload — beside
 * `VIEW_KEY`, because those two are how you like to work. Project, subject and
 * due are per-question and start empty every time; if that turns out wrong the
 * upgrade is the URL (`?n=`'s grammar), which is also what would make ⌘-click
 * and a restored tab carry a filter, and it is not worth building until it is
 * asked for.
 *
 * `oculus-tasks-scope` is the dead key left by the strip this replaced. Nothing
 * migrates it and nothing cleans it up — the reader simply stopped existing, as
 * `oculus-project-view`'s did before it.
 */
const STATUS_KEY = "oculus-tasks-status";

const VIEWS = [
  { value: "board", label: "Board" },
  { value: "table", label: "Table" },
] as const satisfies ReadonlyArray<{ value: TaskView; label: string }>;

function isView(v: string | null): v is TaskView {
  return v === "board" || v === "table";
}

function storedStatus(): readonly string[] {
  try {
    const raw = localStorage.getItem(STATUS_KEY);
    if (!raw) return DEFAULT_FILTER.status;
    const parsed: unknown = JSON.parse(raw);
    // An empty set would be a board with no columns and no way back but the
    // toolbar, and a stored value is the one input nothing validated on the
    // way in.
    if (!Array.isArray(parsed) || parsed.length === 0) return DEFAULT_FILTER.status;
    const ids = parsed.filter((v): v is string => typeof v === "string");
    return ids.length > 0 ? ids : DEFAULT_FILTER.status;
  } catch {
    return DEFAULT_FILTER.status;
  }
}

export default function TasksPage() {
  const [view, setView] = useState<TaskView>(() => {
    const stored = localStorage.getItem(VIEW_KEY);
    return isView(stored) ? stored : "board";
  });
  const [filter, setFilter] = useState<TaskFilter>(() => ({
    ...DEFAULT_FILTER,
    status: storedStatus(),
  }));

  useEffect(() => {
    localStorage.setItem(VIEW_KEY, view);
  }, [view]);

  useEffect(() => {
    localStorage.setItem(STATUS_KEY, JSON.stringify(filter.status));
  }, [filter.status]);

  // Not `projectsStore`: that holds one *open project* and its tasks, which is
  // the wrong shape for a view that spans every project and includes tasks
  // that belong to none. The hook refreshes on `PROJECTS_UPDATED_EVENT`, the
  // same door a click here and a write the chat agent made through
  // `oculus task` both arrive by.
  //
  // One read, always everything: the filter is a predicate over this list, not
  // a query per question.
  const { tasks, projects, projectById, loaded } = useTaskList("all");

  // Writes still go through the store's wrappers, as every other page's do —
  // they are the lib call plus the bookkeeping a re-read cannot express, and
  // the refresh is the event's.
  const moveTask = useProjectsStore((s) => s.moveTask);
  const createTask = useProjectsStore((s) => s.createTask);
  const refileTask = useProjectsStore((s) => s.refileTask);

  /** One clock for the whole pass — a `Date.now()` per row could put two tasks
   *  either side of "overdue" in the same render. */
  const shown = useMemo(() => {
    const now = Date.now();
    return tasks.filter((t) => matchesFilter(t, filter, projectById, now));
  }, [tasks, filter, projectById]);

  const columns = useMemo(() => filteredColumns(filter), [filter]);

  /**
   * The one move this page makes, wherever it comes from — a card dragged
   * into another column, or a status pill picked in the table.
   *
   * It **appends** to the destination column, because there is no manual order
   * to insert into: `appendNeighbour` finds the last task of this task's own
   * project sitting there, and `afterId: null` makes that `last + 1`. Where
   * the card then *draws* is wherever the column's sort puts it — or nowhere,
   * if the status it just took is filtered out, which is the honest readout of
   * a move made under a filter.
   *
   * Against the whole list, not the filtered one: `position` is a fact about
   * every card in that column, and a neighbour hidden by the filter is still a
   * neighbour.
   */
  const move = useCallback(
    (task: DbTaskWithProject, columnId: string) => {
      const before = appendNeighbour(tasks, task, columnId);
      moveTask(task.id, columnId, before, null).catch((e) =>
        console.error("move task failed", e),
      );
    },
    [moveTask, tasks],
  );

  /**
   * Filing a task somewhere else, from the table's Project cell.
   *
   * The whole write is `refileTask`'s: it maps the column across by kind,
   * carries the task's subtasks along and appends at the destination's end
   * (`app/src/lib/projects.ts`). Nothing here has to re-read — under a project
   * filter the row simply leaves the list when the event lands, which is the
   * honest readout of what just happened.
   */
  const refile = useCallback(
    (task: DbTaskWithProject, projectId: number | null) => {
      refileTask(task.id, projectId).catch((e) => console.error("refile task failed", e));
    },
    [refileTask],
  );

  const create = useCallback(
    (projectId: number | null, title: string) => {
      // No `columnId`: `createTask` files it in the first column of the
      // destination's board — see `NewTaskButton`.
      createTask({ projectId, title }).catch((e) =>
        console.error("create task failed", e),
      );
    },
    [createTask],
  );

  const narrowed = isFiltered(filter);
  const done = shown.filter((t) => t.done_at != null).length;

  /** What an empty list says. Under a filter it names the filter — "you have
   *  no tasks" over a filtered-empty list is the same lie the scope strip used
   *  to tell about Unfiled. */
  const empty = narrowed
    ? "Nothing matches these filters — widen one, or clear them back to Any."
    : "No tasks yet — add one above, or break a project down on its board.";

  return (
    <div className="flex h-full flex-col">
      <SectionHeader>
        <span className="flex-1" />
        {/* `done/total` is a readout of everything you have on, so it stops
            being one the moment a filter is up: defaulting to Todo it would
            read "0/12 done", true of the rows on screen and nonsense as a
            summary. Filtered, it says how many rows there are instead. */}
        {narrowed ? (
          <span className="shrink-0 text-[11px] tabular-nums text-muted-foreground">
            {shown.length} shown
          </span>
        ) : (
          tasks.length > 0 && (
            <span className="shrink-0 text-[11px] tabular-nums text-muted-foreground">
              {done}/{tasks.length} done
            </span>
          )
        )}
        <NewTaskButton projects={projects} onCreate={create} />
      </SectionHeader>

      <div className="shrink-0 flex h-9 items-center gap-2.5 px-5">
        <PillTabs tabs={VIEWS} value={view} onChange={setView} />
        <span className="flex-1" />
        <TaskFilters filter={filter} projects={projects} onChange={setFilter} />
      </div>

      <div className="min-h-0 flex-1">
        {!loaded ? (
          // The difference between "nothing to show" and "not read yet": an
          // empty board would otherwise say you have no tasks for the beat the
          // query takes.
          <div className="flex h-full items-center justify-center px-6">
            <p className="text-xs text-muted-foreground">Loading…</p>
          </div>
        ) : view === "board" && shown.length === 0 ? (
          // A row of column headings with nothing under any of them is not an
          // empty state, so the board stands aside for one.
          <div className="flex h-full items-start justify-center px-6 py-16">
            <p className="max-w-sm text-center text-xs text-muted-foreground">{empty}</p>
          </div>
        ) : view === "board" ? (
          <TasksBoard
            tasks={shown}
            columns={columns}
            projectById={projectById}
            onMove={move}
          />
        ) : (
          <TasksTable
            tasks={shown}
            projects={projects}
            projectById={projectById}
            empty={empty}
            onMove={move}
            onRefile={refile}
          />
        )}
      </div>
    </div>
  );
}
