import { ProjectIcon } from "@/components/projects/icons/ProjectIcon";
import { useStoredState } from "@/hooks/ui/useStoredState";
import { useCallback, useEffect, useMemo, useState } from "react";
import { Link, Navigate, useNavigate, useParams } from "react-router-dom";
import { PillTabs } from "@/components/ui/table/PillTabs";
import { ViewTabs } from "@/components/ui/table/ViewTabs";
import { ProjectBoard } from "@/components/projects/board/ProjectBoard";
import { ProjectMenu } from "@/components/projects/lists/ProjectMenu";
import { ProjectOverview } from "@/components/projects/page/ProjectOverview";
import { ProjectTable } from "@/components/projects/lists/ProjectTable";
import {
  ProjectTimeline,
  TIMELINE_ZOOM_KEY,
  TimelineZoomControl,
  isTimelineZoom,
  type TimelineZoom,
} from "@/components/projects/timeline/ProjectTimeline";
import { projectParents } from "@/components/projects/nav/projectBreadcrumbs";
import { Breadcrumb } from "@/components/ui/Breadcrumb";
import { boardProgress, taskTree } from "@/components/projects/tasks/taskTree";
import { useProjectActions } from "@/components/projects/lists/useProjectActions";
import { PageHeader, PaneTrail } from "@/components/tabs/PaneHeader";
import { SideNavCollapseToggle } from "@/components/ui/layout/SideNav";
import { useTasksPanel } from "@/layouts/TasksLayout";
import { shortcut } from "@/lib/platform";
import { PROJECTS_UPDATED_EVENT } from "@/lib/planning/projects";
import { useProjectsStore } from "@/stores/planning/projectsStore";
import { useWindowEvent } from "@/hooks/backend/useEvents";

/** Overview ("what was this again") and Tasks ("what now"). */
type ProjectTab = "overview" | "tasks";

/** Three shapes of the same task rows. */
type TaskView = "board" | "table" | "timeline";

const TAB_KEY = "oculus-project-tab";
const VIEW_KEY = "oculus-project-view";

const TABS = [
  { value: "overview", label: "Overview" },
  { value: "tasks", label: "Tasks" },
] as const satisfies ReadonlyArray<{ value: ProjectTab; label: string }>;

const TASK_VIEWS = [
  { value: "board", label: "Board" },
  { value: "table", label: "Table" },
  { value: "timeline", label: "Timeline" },
] as const satisfies ReadonlyArray<{ value: TaskView; label: string }>;

function isTab(v: string | null): v is ProjectTab {
  return v === "overview" || v === "tasks";
}

/** An unrecognised stored view falls back to Board rather than rendering
 *  nothing. */
function isView(v: string | null): v is TaskView {
  return v === "board" || v === "table" || v === "timeline";
}

/** A shared page header above Overview and the three task views. */
export default function ProjectPage() {
  const { projectId } = useParams();
  const id = Number(projectId);
  const navigate = useNavigate();
  // The nav column's fold toggle; null in the side panel, outside the layout.
  const panel = useTasksPanel();

  const project = useProjectsStore((s) => s.projects.find((p) => p.id === id) ?? null);
  const tasks = useProjectsStore((s) => s.tasks);
  const loadProjects = useProjectsStore((s) => s.loadProjects);
  const openProject = useProjectsStore((s) => s.open);
  const reload = useProjectsStore((s) => s.reload);
  const moveTask = useProjectsStore((s) => s.moveTask);
  const createTask = useProjectsStore((s) => s.createTask);
  const updateProject = useProjectsStore((s) => s.updateProject);
  const actions = useProjectActions();

  const [listed, setListed] = useState(false);

  const [tab, setTab] = useStoredState<ProjectTab>(TAB_KEY, (stored) =>
    isTab(stored) ? stored : "overview",
  );
  const [view, setView] = useStoredState<TaskView>(VIEW_KEY, (stored) =>
    isView(stored) ? stored : "board",
  );

  // The page's state, because its control sits in the view row; persisted.
  const [zoom, setZoom] = useStoredState<TimelineZoom>(TIMELINE_ZOOM_KEY, (stored) =>
    isTimelineZoom(stored) ? stored : "week",
  );


  // `status: "all"` so an archived project opened by its link still resolves.
  useEffect(() => {
    setListed(false);
    loadProjects({ status: "all" }).finally(() => setListed(true));
  }, [loadProjects, id]);

  useEffect(() => {
    if (Number.isFinite(id)) void openProject(id);
  }, [openProject, id]);

  // Writes from elsewhere (the agent, another page) arrive as this event.
  useWindowEvent(PROJECTS_UPDATED_EVENT, () => void reload());

  const nodes = useMemo(() => taskTree(tasks), [tasks]);
  const progress = useMemo(() => boardProgress(nodes), [nodes]);

  const handleMove = useCallback(
    (taskId: number, columnId: string, before: number | null, after: number | null) => {
      moveTask(taskId, columnId, before, after).catch((e) =>
        console.error("move task failed", e),
      );
    },
    [moveTask],
  );

  const handleCreate = useCallback(
    (input: { title: string; columnId: string; parentId?: number }) => {
      createTask({
        projectId: id,
        title: input.title,
        columnId: input.columnId,
        parentId: input.parentId ?? null,
      }).catch((e) => console.error("create task failed", e));
    },
    [createTask, id],
  );

  /** Every field the Overview edits. */
  const handlePatch = useCallback(
    (patch: Parameters<typeof updateProject>[1]) => {
      updateProject(id, patch).catch((e) => console.error("update project failed", e));
    },
    [updateProject, id],
  );

  if (!Number.isFinite(id)) return <Navigate to="/projects" replace />;

  if (!project) {
    return (
      <div className="flex h-full items-center justify-center px-6">
        {listed ? (
          <p className="text-xs text-muted-foreground">
            That project is gone.{" "}
            <Link to="/projects" className="text-brand hover:underline">
              Back to Projects
            </Link>
          </p>
        ) : (
          <p className="text-xs text-muted-foreground">Loading…</p>
        )}
      </div>
    );
  }

  const titleRow = (
    <PageHeader>
      {panel && <SideNavCollapseToggle collapsed={panel.collapsed} onToggle={panel.toggle} shortcut={shortcut("mod+alt+B")} />}
      <PaneTrail>
        <Breadcrumb items={[...projectParents(project), { label: project.name, icon: <ProjectIcon project={project} size={14} /> }]} />
      </PaneTrail>
      {tab === "tasks" && <span className="shrink-0 text-[11px] tabular-nums text-muted-foreground">{progress.done}/{progress.total} done</span>}
      <ProjectMenu
        project={project}
        className="shrink-0"
        onRename={(name) => actions.onRename(project, name)}
        onArchive={() => {
          // Archived or deleted, the board has nothing left to show.
          actions.onArchive(project);
          navigate("/projects");
        }}
        onUnarchive={() => actions.onUnarchive(project)}
        onDelete={() => {
          actions.onDelete(project);
          navigate("/projects");
        }}
      />
    </PageHeader>
  );

  return (
    <div className="flex h-full flex-col">
      {titleRow}
      {/* Tabs alone on the rule the active tab underlines. */}
      <div className="shrink-0 flex items-end border-b border-border-subtle px-6 pt-2">
        <ViewTabs tabs={TABS} value={tab} onChange={setTab} />
      </div>

      {/* Own row, not beside the name, where they read as more crumbs. The
          timeline's zoom sits with the view it controls. */}
      {tab === "tasks" && (
        <div className="shrink-0 flex h-9 items-center gap-2.5 px-5">
          <PillTabs tabs={TASK_VIEWS} value={view} onChange={setView} />
          <span className="flex-1" />
          {view === "timeline" && <TimelineZoomControl value={zoom} onChange={setZoom} />}
        </div>
      )}

      <div className="min-h-0 flex-1">
        {tab === "overview" && (
          <ProjectOverview project={project} nodes={nodes} onPatch={handlePatch} onRename={(name) => actions.onRename(project, name)} />
        )}
        {tab === "tasks" && view === "board" && (
          <ProjectBoard
            project={project}
            nodes={nodes}
            onMove={handleMove}
            onCreate={handleCreate}
          />
        )}
        {tab === "tasks" && view === "table" && (
          <ProjectTable
            project={project}
            nodes={nodes}
            onMove={handleMove}
            onCreate={handleCreate}
          />
        )}
        {tab === "tasks" && view === "timeline" && (
          <ProjectTimeline project={project} nodes={nodes} zoom={zoom} />
        )}
      </div>
    </div>
  );
}
