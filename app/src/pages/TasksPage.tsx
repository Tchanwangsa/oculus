import { useCallback, useEffect, useState } from "react";
import { PillTabs } from "@/components/ui/PillTabs";
import { ViewTabs } from "@/components/ui/ViewTabs";
import { NewTaskButton } from "@/components/projects/NewTaskButton";
import { TasksBoard } from "@/components/projects/TasksBoard";
import { TasksTable } from "@/components/projects/TasksTable";
import { appendNeighbour } from "@/components/projects/universalTasks";
import { useTaskList, type TaskScope } from "@/hooks/useTaskList";
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
 * The chrome is `ProjectPage`'s: the scope tabs alone on the container's
 * bottom rule, a fixed `h-12` toolbar, then the quieter `PillTabs` strip for
 * the two views — the same two rows those three views sit under there, so
 * switching view cannot jolt the work below.
 */
type TaskView = "board" | "table";

const VIEW_KEY = "oculus-tasks-view";
const SCOPE_KEY = "oculus-tasks-scope";

/**
 * All, or only what is filed nowhere.
 *
 * The top-level strip, because these are two questions rather than two shapes
 * of one answer — "what is there to do" against "what have I not decided about
 * yet" — which is the same split `ProjectPage` makes with Overview / Tasks.
 * Each is one query (`getAllTasks` / `getUnfiledTasks`), so the filtering is
 * SQL's rather than a predicate over a list the page holds twice.
 */
const SCOPES = [
  { value: "all", label: "All tasks" },
  { value: "unfiled", label: "Unfiled" },
] as const satisfies ReadonlyArray<{ value: TaskScope; label: string }>;

const VIEWS = [
  { value: "board", label: "Board" },
  { value: "table", label: "Table" },
] as const satisfies ReadonlyArray<{ value: TaskView; label: string }>;

function isView(v: string | null): v is TaskView {
  return v === "board" || v === "table";
}

function isScope(v: string | null): v is TaskScope {
  return v === "all" || v === "unfiled";
}

const EMPTY: Record<TaskScope, string> = {
  all: "No tasks yet — add one above, or break a project down on its board.",
  unfiled: "Nothing unfiled. Every task you have belongs to a project.",
};

export default function TasksPage() {
  const [scope, setScope] = useState<TaskScope>(() => {
    const stored = localStorage.getItem(SCOPE_KEY);
    return isScope(stored) ? stored : "all";
  });
  const [view, setView] = useState<TaskView>(() => {
    const stored = localStorage.getItem(VIEW_KEY);
    return isView(stored) ? stored : "board";
  });

  useEffect(() => {
    localStorage.setItem(SCOPE_KEY, scope);
  }, [scope]);

  useEffect(() => {
    localStorage.setItem(VIEW_KEY, view);
  }, [view]);

  // Not `projectsStore`: that holds one *open project* and its tasks, which is
  // the wrong shape for a view that spans every project and includes tasks
  // that belong to none. The hook refreshes on `PROJECTS_UPDATED_EVENT`, the
  // same door a click here and a write the chat agent made through
  // `oculus task` both arrive by.
  const { tasks, projects, projectById, loaded } = useTaskList(scope);

  // Writes still go through the store's wrappers, as every other page's do —
  // they are the lib call plus the bookkeeping a re-read cannot express, and
  // the refresh is the event's.
  const moveTask = useProjectsStore((s) => s.moveTask);
  const createTask = useProjectsStore((s) => s.createTask);
  const refileTask = useProjectsStore((s) => s.refileTask);

  /**
   * The one move this page makes, wherever it comes from — a card dragged
   * into another kind, or a status pill picked in the table.
   *
   * It **appends** to the destination column, because there is no manual order
   * to insert into: `appendNeighbour` finds the last task of this task's own
   * project sitting there, and `afterId: null` makes that `last + 1`. Where
   * the card then *draws* is wherever the column's sort puts it.
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
   * (`app/src/lib/projects.ts`). Nothing here has to re-read — on the Unfiled
   * scope the row simply leaves the list when the event lands, which is the
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

  const done = tasks.filter((t) => t.done_at != null).length;

  return (
    <div className="flex h-full flex-col">
      {/* Tabs alone on the rule the active tab underlines. */}
      <div className="shrink-0 flex items-end border-b border-border-subtle px-5 pt-4">
        <ViewTabs tabs={SCOPES} value={scope} onChange={setScope} />
      </div>

      {/* Fixed-height toolbar: what the page is, then how it is going and what
          you can do about it. */}
      <div className="shrink-0 flex h-12 items-center gap-2.5 px-5">
        {/* Not an `<h1>`: the tab strip already names the page, and a heading
            here would be a second title on the same rule. */}
        <span className="min-w-0 font-display text-[13px] font-semibold tracking-tight text-foreground">
          Tasks
        </span>
        <span className="flex-1" />
        {tasks.length > 0 && (
          <span className="shrink-0 text-[11px] tabular-nums text-muted-foreground">
            {done}/{tasks.length} done
          </span>
        )}
        <NewTaskButton projects={projects} onCreate={create} />
      </div>

      <div className="shrink-0 flex h-9 items-center gap-2.5 px-5">
        <PillTabs tabs={VIEWS} value={view} onChange={setView} />
      </div>

      <div className="min-h-0 flex-1">
        {!loaded ? (
          // The difference between "nothing to show" and "not read yet": an
          // empty board would otherwise say you have no tasks for the beat the
          // query takes.
          <div className="flex h-full items-center justify-center px-6">
            <p className="text-xs text-muted-foreground">Loading…</p>
          </div>
        ) : view === "board" ? (
          <TasksBoard tasks={tasks} projectById={projectById} onMove={move} />
        ) : (
          <TasksTable
            tasks={tasks}
            projects={projects}
            projectById={projectById}
            empty={EMPTY[scope]}
            onMove={move}
            onRefile={refile}
          />
        )}
      </div>
    </div>
  );
}
