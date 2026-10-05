import { useStoredState } from "@/hooks/useStoredState";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { Link, Navigate, useNavigate, useParams } from "react-router-dom";
import { Badge } from "@/components/ui/badge";
import { PillTabs } from "@/components/ui/PillTabs";
import { ViewTabs } from "@/components/ui/ViewTabs";
import { ProjectBoard } from "@/components/projects/ProjectBoard";
import { ProjectMenu } from "@/components/projects/ProjectMenu";
import { ProjectOverview } from "@/components/projects/ProjectOverview";
import { ProjectTable } from "@/components/projects/ProjectTable";
import {
  ProjectTimeline,
  TIMELINE_ZOOM_KEY,
  TimelineZoomControl,
  isTimelineZoom,
  type TimelineZoom,
} from "@/components/projects/ProjectTimeline";
import { DueChip } from "@/components/projects/TaskMarks";
import { projectHref } from "@/components/projects/projectHref";
import { ProjectCrumbs } from "@/components/projects/ProjectCrumbs";
import { boardProgress, taskTree } from "@/components/projects/taskTree";
import { useProjectActions } from "@/components/projects/useProjectActions";
import { PaneHeaderRow, PaneTrail, useInSidePanel } from "@/components/tabs/PaneHeader";
import { cn } from "@/lib/utils";
import { PROJECTS_UPDATED_EVENT, type DbProject } from "@/lib/projects";
import { useProjectsStore } from "@/stores/projectsStore";
import { useWindowEvent } from "@/hooks/useEvents";

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

/**
 * One project: its Overview, and its tasks read three ways. Chrome follows
 * `SyncPage`: the tab strip on the bottom rule, then a fixed `h-12` toolbar
 * (scope left, state and actions right) so switching views doesn't jolt the
 * content. On Tasks a second row carries the view tabs.
 */
export default function ProjectPage() {
  const { projectId } = useParams();
  const id = Number(projectId);
  const navigate = useNavigate();
  const inSide = useInSidePanel();

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

  // The title row; in the side panel it leads, as the panel's header.
  const titleRow = (
    <PaneHeaderRow className="shrink-0 flex h-12 items-center gap-2.5 px-5">
      <PaneTrail>
        <nav
          aria-label="Breadcrumb"
          className="flex shrink-0 items-center gap-2.5 text-[11px] text-muted-foreground"
        >
          <ProjectCrumbs project={project} />
        </nav>
        <ProjectTitle project={project} onRename={(name) => actions.onRename(project, name)} />

        {project.status !== "active" && (
          <Badge variant="secondary" className="shrink-0 text-[11px]">
            Archived
          </Badge>
        )}
      </PaneTrail>

      {!inSide && <span className="flex-1" />}

      {project.due_at && (
        <span className="flex shrink-0 items-center gap-1 text-[11px] text-muted-foreground">
          Due <DueChip dueAt={project.due_at} />
        </span>
      )}
      {/* Not on Overview, whose Properties list shows the same fraction. */}
      {tab === "tasks" && (
        <span className="shrink-0 text-[11px] tabular-nums text-muted-foreground">
          {progress.done}/{progress.total} done
        </span>
      )}
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
    </PaneHeaderRow>
  );

  return (
    <div className="flex h-full flex-col">
      {inSide && titleRow}
      {/* Tabs alone on the rule the active tab underlines. */}
      <div className="shrink-0 flex items-end border-b border-border-subtle px-5 pt-4">
        <ViewTabs tabs={TABS} value={tab} onChange={setTab} />
      </div>
      {!inSide && titleRow}

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
          <ProjectOverview project={project} nodes={nodes} onPatch={handlePatch} />
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

/**
 * The project's name, editable in place (`ProjectMenu`'s rename dialog serves
 * list rows, which are links). Empty or unchanged reverts rather than commits.
 */
function ProjectTitle({
  project,
  onRename,
}: {
  project: DbProject;
  onRename: (name: string) => void;
}) {
  const navigate = useNavigate();
  const inSide = useInSidePanel();
  const [editing, setEditing] = useState(false);
  const [draft, setDraft] = useState(project.name);
  const ref = useRef<HTMLInputElement>(null);

  useEffect(() => {
    if (editing) ref.current?.select();
  }, [editing]);

  const commit = () => {
    setEditing(false);
    const name = draft.trim();
    if (!name || name === project.name) {
      setDraft(project.name);
      return;
    }
    onRename(name);
    // `tabInfo` titles a tab from `projectHref`'s `?n=`, so re-navigate;
    // replace keeps the back arrow where it was.
    navigate(projectHref({ id: project.id, name }), { replace: true });
  };

  const shared =
    "min-w-0 font-display text-[13px] font-semibold tracking-tight text-foreground";

  if (editing) {
    return (
      <input
        ref={ref}
        value={draft}
        aria-label="Project name"
        onChange={(e) => setDraft(e.target.value)}
        onBlur={commit}
        onKeyDown={(e) => {
          if (e.key === "Enter") commit();
          if (e.key === "Escape") {
            setDraft(project.name);
            setEditing(false);
          }
        }}
        className={cn(shared, "w-52 rounded-md bg-transparent outline-none")}
      />
    );
  }

  return (
    // Not an <h1>: the tab strip already names the page.
    <span
      tabIndex={0}
      title={project.name}
      onClick={() => {
        setDraft(project.name);
        setEditing(true);
      }}
      onKeyDown={(e) => {
        if (e.key === "Enter") {
          e.preventDefault();
          setDraft(project.name);
          setEditing(true);
        }
      }}
      // In the side panel the row's trail scrolls instead of truncating.
      className={cn(
        shared,
        inSide ? "shrink-0 whitespace-nowrap" : "truncate",
        "cursor-text rounded-md outline-none",
      )}
    >
      {project.name}
    </span>
  );
}
