import { useCallback, useMemo, useState } from "react";
import { useSearchParams } from "react-router-dom";
import { TasksBoard } from "@/components/projects/board/TasksBoard";
import { DisplayMenu } from "@/components/projects/display/DisplayMenu";
import { DISPLAY_KEY, readDisplay, writeDisplay } from "@/components/projects/display/settings";
import { TasksTable } from "@/components/projects/lists/TasksTable";
import { NewTaskButton } from "@/components/projects/nav/NewTaskButton";
import { useCreateTask } from "@/components/projects/nav/useCreateTask";
import { SectionHeader } from "@/components/projects/page/SectionHeader";
import {
  DEFAULT_FILTER,
  filteredColumns,
  isFiltered,
  matchesFilter,
  type TaskFilter,
} from "@/components/projects/tasks/filterRules";
import { ProjectFilter } from "@/components/projects/tasks/ProjectFilter";
import { SubjectFilter } from "@/components/projects/tasks/SubjectFilter";
import { TaskFilters } from "@/components/projects/tasks/TaskFilters";
import { TaskSearch } from "@/components/projects/tasks/TaskSearch";
import { sortTasks } from "@/components/projects/tasks/taskSort";
import {
  matchesSearch,
  matchesView,
  parseTagParam,
  parseView,
  viewLabel,
} from "@/components/projects/tasks/taskViews";
import { appendNeighbour } from "@/components/projects/tasks/universalTasks";
import { useTaskList } from "@/hooks/data/useTaskList";
import { useTaskTags } from "@/hooks/data/useTaskTags";
import { useStoredState } from "@/hooks/ui/useStoredState";
import { useTasksPanel } from "@/layouts/TasksLayout";
import { createTaskTag, type DbTaskTag, type DbTaskWithProject } from "@/lib/planning/projects";
import { useProjectsStore } from "@/stores/planning/projectsStore";

/**
 * Every task across every project, plus unfiled ones.
 *
 * What is listed is the nav column's view and tag (`?view=`, `?tag=`), the
 * search, and the Filter, Project and Subject pills, ANDed; how it is drawn is
 * the Display menu (`app/src/components/projects/display/`). Spanning projects costs manual
 * order: `position` is only comparable inside one project's column, so lists
 * are sorted and a board drag within a column is a no-op
 * (`app/src/components/projects/tasks/universalTasks.ts`).
 */
export default function TasksPage() {
  const [params] = useSearchParams();
  const view = parseView(params.get("view"));
  const tagParam = parseTagParam(params.get("tag"));

  const [display, setDisplay] = useStoredState(DISPLAY_KEY, readDisplay, writeDisplay);
  const [pickedFilter, setFilter] = useState<TaskFilter>(DEFAULT_FILTER);
  const [query, setQuery] = useState("");
  const panel = useTasksPanel();

  // Not `projectsStore`, which holds one open project. One read of everything;
  // the view, search and filter are predicates over it. Refreshes on
  // `PROJECTS_UPDATED_EVENT`.
  const { tasks, projects, projectById, loaded } = useTaskList("all");
  const { tags, byId: tagById, loaded: tagsLoaded } = useTaskTags();
  // A tag deleted from elsewhere leaves a stale link; it selects nothing.
  const tagId = tagParam != null && (!tagsLoaded || tagById.has(tagParam)) ? tagParam : null;
  const tag: DbTaskTag | undefined = tagId != null ? tagById.get(tagId) : undefined;
  // Likewise a deleted tag still ticked in the filter popover.
  const filter = useMemo(
    () =>
      !tagsLoaded || pickedFilter.tags.every((id) => tagById.has(id))
        ? pickedFilter
        : { ...pickedFilter, tags: pickedFilter.tags.filter((id) => tagById.has(id)) },
    [pickedFilter, tagById, tagsLoaded],
  );

  // Writes go through the store's wrappers; the refresh is the event's.
  const moveTask = useProjectsStore((s) => s.moveTask);
  const updateTask = useProjectsStore((s) => s.updateTask);
  const refileTask = useProjectsStore((s) => s.refileTask);
  const create = useCreateTask();

  /** One clock for the pass, so "overdue" is consistent across rows. */
  const shown = useMemo(() => {
    const now = Date.now();
    const tagName = (id: number) => tagById.get(id)?.name;
    const rows = tasks.filter(
      (t) =>
        (display.showCompleted || t.done_at == null) &&
        matchesView(t, view) &&
        (tagId == null || t.tag_ids.includes(tagId)) &&
        matchesSearch(t, query, tagName) &&
        matchesFilter(t, filter, projectById, now),
    );
    return sortTasks(rows, display.sort, projectById);
  }, [tasks, display.showCompleted, display.sort, view, tagId, query, filter, projectById, tagById]);

  const columns = useMemo(() => filteredColumns(filter), [filter]);

  /** A drag or a status pick. Appends to the destination column (there is no
   *  manual order here), computed against the whole list, not the filtered one:
   *  a hidden neighbour is still a neighbour for `position`. */
  const move = useCallback(
    (task: DbTaskWithProject, columnId: string) => {
      const before = appendNeighbour(tasks, task, columnId);
      moveTask(task.id, columnId, before, null).catch((e) => console.error("move task failed", e));
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

  const actions = useMemo(
    () => ({
      onMove: move,
      onRefile: refile,
      onTags: (task: DbTaskWithProject, tagIds: number[]) => {
        updateTask(task.id, { tagIds }).catch((e) => console.error("update task tags failed", e));
      },
      onCreateTag: (name: string) => createTaskTag(name),
    }),
    [move, refile, updateTask],
  );

  const title = tag ? (view === "all" ? tag.name : `${tag.name} · ${viewLabel(view)}`) : viewLabel(view);
  const empty =
    tasks.length === 0
      ? "No tasks yet — add one with New task, or break a project down on its board."
      : isFiltered(filter) || query.trim() || tagId != null || view !== "all" || !display.showCompleted
        ? "Nothing matches — change the view, the search or the filters."
        : "Nothing to show.";

  return (
    <div className="flex h-full flex-col">
      <SectionHeader title={title}>
        <span className="flex-1" />
        {/* The nav column holds New task; folded away, it moves here. */}
        {panel?.collapsed && <NewTaskButton projects={projects} onCreate={create} />}
        <span className="shrink-0 text-[12px] tabular-nums text-muted-foreground">
          {shown.length} {shown.length === 1 ? "task" : "tasks"}
        </span>
      </SectionHeader>

      <div className="shrink-0 flex h-11 items-center gap-2 px-5">
        <TaskSearch value={query} onChange={setQuery} />
        <span className="flex-1" />
        <TaskFilters filter={filter} tags={tags} onChange={setFilter} />
        <ProjectFilter filter={filter} projects={projects} onChange={setFilter} />
        <SubjectFilter filter={filter} onChange={setFilter} />
        <DisplayMenu display={display} onChange={setDisplay} />
      </div>

      <div className="min-h-0 flex-1">
        {!loaded ? (
          <div className="flex h-full items-center justify-center px-6">
            <p className="text-xs text-muted-foreground">Loading…</p>
          </div>
        ) : display.view === "board" && shown.length === 0 ? (
          <div className="flex h-full items-start justify-center px-6 py-16">
            <p className="max-w-sm text-center text-xs text-muted-foreground">{empty}</p>
          </div>
        ) : display.view === "board" ? (
          <TasksBoard tasks={shown} columns={columns} projectById={projectById} tagById={tagById} onMove={move} />
        ) : (
          <TasksTable
            tasks={shown}
            projects={projects}
            projectById={projectById}
            tags={tags}
            tagById={tagById}
            display={display}
            onSort={(sort) => setDisplay((d) => ({ ...d, sort }))}
            empty={empty}
            actions={actions}
          />
        )}
      </div>
    </div>
  );
}
