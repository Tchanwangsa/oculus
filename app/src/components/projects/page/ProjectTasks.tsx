import { useMemo, useState } from "react";
import type { DbProject, DbTaskWithProject } from "@/lib/planning/projects";
import { useTaskTags } from "@/hooks/data/useTaskTags";
import { useStoredState } from "@/hooks/ui/useStoredState";
import { DisplayMenu } from "../display/DisplayMenu";
import { PROJECT_DISPLAY, PROJECT_DISPLAY_KEY, readDisplay, writeDisplay, type TasksDisplay } from "../display/settings";
import { DEFAULT_FILTER, filteredColumns, matchesFilter } from "../tasks/filterRules";
import { boardColumns, taskTree } from "../tasks/taskTree";
import { matchesSearch } from "../tasks/taskViews";
import { sortTasks } from "../tasks/taskSort";
import { TaskFilters } from "../tasks/TaskFilters";
import { TaskToolbar } from "../tasks/TaskToolbar";
import { TasksView } from "../tasks/TasksView";
import { useTaskActions } from "../tasks/useTaskActions";
import { ProjectTimeline, type TimelineZoom } from "../timeline/ProjectTimeline";

/** A project supplies the scope and statuses to the shared task views. */
export function ProjectTasks({ project, tasks, projects, projectById, view, onView, zoom, onCreate }: {
  project: DbProject;
  tasks: DbTaskWithProject[];
  projects: DbProject[];
  projectById: Map<number, DbProject>;
  view: "board" | "table" | "timeline";
  onView: (view: "board" | "table" | "timeline") => void;
  zoom: TimelineZoom;
  onCreate: (input: { title: string; columnId: string }) => void;
}) {
  const [stored, setStored] = useStoredState(PROJECT_DISPLAY_KEY, (raw) => readDisplay(raw, PROJECT_DISPLAY), writeDisplay);
  const [query, setQuery] = useState("");
  const [filter, setFilter] = useState(DEFAULT_FILTER);
  const { tags, byId: tagById } = useTaskTags();
  const actions = useTaskActions(tasks);
  const display = useMemo(() => ({ ...stored, view: view === "timeline" ? stored.view : view }), [stored, view]);
  const defaults = useMemo(() => ({ ...PROJECT_DISPLAY, view: display.view }), [display.view]);
  const onDisplay = (next: TasksDisplay) => { setStored(next); onView(next.view); };
  const ownColumns = useMemo(() => boardColumns(project), [project]);
  const columns = useMemo(() => filteredColumns(filter, ownColumns), [filter, ownColumns]);
  const shown = useMemo(() => {
    const now = Date.now();
    return sortTasks(tasks.filter((task) =>
      (display.showCompleted || task.done_at == null) &&
      matchesSearch(task, query, (id) => tagById.get(id)?.name) &&
      matchesFilter(task, filter, projectById, now, project)), display.sort, projectById, ownColumns);
  }, [tasks, display.showCompleted, display.sort, query, tagById, filter, projectById, project, ownColumns]);
  const nodes = useMemo(() => taskTree(shown), [shown]);
  return <div className="flex h-full min-h-0 flex-col">
    <TaskToolbar value={query} onChange={setQuery}>
      <TaskFilters filter={filter} tags={tags} onChange={setFilter} columns={ownColumns} showArchived={false} />
      {view !== "timeline" && <DisplayMenu display={display} onChange={onDisplay} defaults={defaults} showLayout={false} />}
    </TaskToolbar>
    <div className="min-h-0 flex-1">
      {view === "timeline" ? <ProjectTimeline project={project} nodes={nodes} zoom={zoom} /> : (
        <TasksView tasks={shown} allTasks={tasks} projects={projects} projectById={projectById}
          tags={tags} tagById={tagById} display={display} onSort={(sort) => onDisplay({ ...display, sort })}
          columns={columns} project={project} onCreate={onCreate} actions={actions}
          empty={tasks.length === 0 ? "No tasks yet — add the first task below." : "No tasks match — change the search, filters or Display options."} />
      )}
    </div>
  </div>;
}
