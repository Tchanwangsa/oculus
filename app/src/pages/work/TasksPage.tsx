import { useStoredState } from "@/hooks/ui/useStoredState";
import { useCallback, useEffect, useMemo, useState } from "react";
import { PillTabs } from "@/components/ui/table/PillTabs";
import { NewTaskButton } from "@/components/projects/nav/NewTaskButton";
import { SectionHeader } from "@/components/projects/page/SectionHeader";
import { TasksBoard } from "@/components/projects/board/TasksBoard";
import { TasksTable } from "@/components/projects/lists/TasksTable";
import {
  DEFAULT_FILTER,
  TaskFilters,
  filteredColumns,
  isFiltered,
  matchesFilter,
  type TaskFilter,
} from "@/components/projects/tasks/TaskFilters";
import {
  UNIVERSAL_COLUMNS,
  appendNeighbour,
} from "@/components/projects/tasks/universalTasks";
import { useTaskList } from "@/hooks/data/useTaskList";
import type { DbTaskWithProject } from "@/lib/planning/projects";
import { useProjectsStore } from "@/stores/planning/projectsStore";

/**
 * Every task across every project, plus unfiled ones — the section's second
 * tab under `SectionHeader`'s `Projects · Tasks` strip.
 *
 * Spanning projects costs manual order: `position` is only comparable inside
 * one project's column, so board columns are sorted and a drag within one is a
 * no-op (`app/src/components/projects/tasks/universalTasks.ts`).
 *
 * Filters' meaning lives in `TaskFilters.tsx`; this page holds their state. The
 * status filter decides which columns the board has, deliberately.
 */
type TaskView = "board" | "table";

const VIEW_KEY = "oculus-tasks-view";

/** Only the status filter persists (beside `VIEW_KEY`); project, subject and
 *  due start empty each time. */
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
    if (!Array.isArray(parsed)) return DEFAULT_FILTER.status;
    // Only ids that still exist, in board order; an empty set would leave a
    // board with no columns.
    const ids = UNIVERSAL_COLUMNS.filter((c) => parsed.includes(c.id)).map((c) => c.id);
    return ids.length > 0 ? ids : DEFAULT_FILTER.status;
  } catch {
    return DEFAULT_FILTER.status;
  }
}

export default function TasksPage() {
  const [view, setView] = useStoredState<TaskView>(VIEW_KEY, (stored) =>
    isView(stored) ? stored : "board",
  );
  const [filter, setFilter] = useState<TaskFilter>(() => ({
    ...DEFAULT_FILTER,
    status: storedStatus(),
  }));


  useEffect(() => {
    localStorage.setItem(STATUS_KEY, JSON.stringify(filter.status));
  }, [filter.status]);

  // Not `projectsStore`, which holds one open project. One read of everything;
  // the filter is a predicate over it. Refreshes on `PROJECTS_UPDATED_EVENT`.
  const { tasks, projects, projectById, loaded } = useTaskList("all");

  // Writes go through the store's wrappers; the refresh is the event's.
  const moveTask = useProjectsStore((s) => s.moveTask);
  const createTask = useProjectsStore((s) => s.createTask);
  const refileTask = useProjectsStore((s) => s.refileTask);

  /** One clock for the pass, so "overdue" is consistent across rows. */
  const shown = useMemo(() => {
    const now = Date.now();
    return tasks.filter((t) => matchesFilter(t, filter, projectById, now));
  }, [tasks, filter, projectById]);

  const columns = useMemo(() => filteredColumns(filter), [filter]);

  /** A drag or a status pick. Appends to the destination column (there is no
   *  manual order here), computed against the whole list, not the filtered one:
   *  a hidden neighbour is still a neighbour for `position`. */
  const move = useCallback(
    (task: DbTaskWithProject, columnId: string) => {
      const before = appendNeighbour(tasks, task, columnId);
      moveTask(task.id, columnId, before, null).catch((e) =>
        console.error("move task failed", e),
      );
    },
    [moveTask, tasks],
  );

  /** From the table's Project cell; the whole write is `refileTask`'s. */
  const refile = useCallback(
    (task: DbTaskWithProject, projectId: number | null) => {
      refileTask(task.id, projectId).catch((e) => console.error("refile task failed", e));
    },
    [refileTask],
  );

  const create = useCallback(
    (projectId: number | null, title: string) => {
      // No `columnId`: `createTask` uses the destination's first column.
      createTask({ projectId, title }).catch((e) =>
        console.error("create task failed", e),
      );
    },
    [createTask],
  );

  const narrowed = isFiltered(filter);
  const done = shown.filter((t) => t.done_at != null).length;

  /** Under a filter, the empty state names the filter. */
  const empty = narrowed
    ? "Nothing matches these filters — widen one, or clear them back to Any."
    : "No tasks yet — add one above, or break a project down on its board.";

  return (
    <div className="flex h-full flex-col">
      <SectionHeader>
        <span className="flex-1" />
        {/* `done/total` summarises everything, so under a filter show a count. */}
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
          <div className="flex h-full items-center justify-center px-6">
            <p className="text-xs text-muted-foreground">Loading…</p>
          </div>
        ) : view === "board" && shown.length === 0 ? (
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
