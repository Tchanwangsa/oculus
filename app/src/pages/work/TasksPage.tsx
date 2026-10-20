import { useTaskFilterState } from "@/components/projects/tasks/useTaskFilterState";
import { useMemo, useState } from "react";
import { useSearchParams } from "react-router-dom";
import { TasksView } from "@/components/projects/tasks/TasksView";
import { DisplayMenu } from "@/components/projects/display/DisplayMenu";
import { DISPLAY_KEY, readDisplay, writeDisplay } from "@/components/projects/display/settings";
import { NewTaskButton } from "@/components/projects/nav/NewTaskButton";
import { useCreateTask } from "@/components/projects/nav/useCreateTask";
import { SectionHeader } from "@/components/projects/page/SectionHeader";
import {
  filteredColumns,
  isFiltered,
  matchesFilter,
} from "@/components/projects/tasks/filterRules";
import { taskFilterCounts } from "@/components/projects/tasks/filterCounts";
import { ProjectFilter } from "@/components/projects/tasks/ProjectFilter";
import { SubjectFilter } from "@/components/projects/tasks/SubjectFilter";
import { TaskFilters } from "@/components/projects/tasks/TaskFilters";
import { TaskToolbar } from "@/components/projects/tasks/TaskToolbar";
import { sortTasks } from "@/components/projects/tasks/taskSort";
import {
  matchesSearch,
  matchesView,
  parseTagParam,
  parseView,
  viewLabel,
} from "@/components/projects/tasks/taskViews";
import { useTaskActions } from "@/components/projects/tasks/useTaskActions";
import { useTaskList } from "@/hooks/data/useTaskList";
import { useTaskTags } from "@/hooks/data/useTaskTags";
import { useStoredState } from "@/hooks/ui/useStoredState";
import { useTasksPanel } from "@/layouts/TasksLayout";
import { type DbTaskTag } from "@/lib/planning/projects";

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
  const [pickedFilter, setFilter] = useTaskFilterState();
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

  const actions = useTaskActions(tasks);
  const create = useCreateTask();

  // Search, nav and completion apply to rows and option counts alike. Each
  // filter menu counts its alternatives before its own selection is applied.
  const candidates = useMemo(() => {
    const tagName = (id: number) => tagById.get(id)?.name;
    return tasks.filter((t) =>
      (display.showCompleted || t.done_at == null) &&
      matchesView(t, view) &&
      (tagId == null || t.tag_ids.includes(tagId)) &&
      matchesSearch(t, query, tagName),
    );
  }, [tasks, display.showCompleted, view, tagId, query, tagById]);

  const { shown, counts } = useMemo(() => {
    const now = Date.now();
    return {
      shown: sortTasks(candidates.filter((t) => matchesFilter(t, filter, projectById, now)), display.sort, projectById),
      counts: taskFilterCounts(candidates, filter, projectById, now),
    };
  }, [candidates, filter, display.sort, projectById]);

  const columns = useMemo(() => filteredColumns(filter), [filter]);

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

      <TaskToolbar value={query} onChange={setQuery}>
        <TaskFilters filter={filter} tags={tags} onChange={setFilter} />
        <ProjectFilter filter={filter} projects={projects} counts={counts} onChange={setFilter} />
        <SubjectFilter filter={filter} counts={counts} onChange={setFilter} />
        <DisplayMenu display={display} onChange={setDisplay} />
      </TaskToolbar>

      <div className="min-h-0 flex-1">
        {!loaded ? (
          <div className="flex h-full items-center justify-center px-6">
            <p className="text-xs text-muted-foreground">Loading…</p>
          </div>
        ) : (
          <TasksView
            columns={columns}
            tasks={shown}
            allTasks={tasks}
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
