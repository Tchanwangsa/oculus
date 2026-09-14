import { useCallback, useEffect, useMemo, useState } from "react";
import { Link, Navigate, useNavigate, useParams } from "react-router-dom";
import { Archive } from "@phosphor-icons/react";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { ViewTabs } from "@/components/ui/ViewTabs";
import { SubjectIcon } from "@/components/subjects/SubjectIcon";
import { ProjectBoard } from "@/components/projects/ProjectBoard";
import { ProjectTable } from "@/components/projects/ProjectTable";
import { ProjectBacklog } from "@/components/projects/ProjectBacklog";
import {
  ProjectTimeline,
  TIMELINE_ZOOM_KEY,
  TimelineZoomControl,
  isTimelineZoom,
  type TimelineZoom,
} from "@/components/projects/ProjectTimeline";
import { DueChip } from "@/components/projects/TaskMarks";
import { boardProgress, taskTree } from "@/components/projects/taskTree";
import { displayCode } from "@/lib/format";
import { PROJECTS_UPDATED_EVENT } from "@/lib/projects";
import { useProjectsStore } from "@/stores/projectsStore";

type ProjectView = "board" | "table" | "backlog" | "timeline";
const VIEW_KEY = "oculus-project-view";

/** The four ways to look at one project, as sibling tabs rather than a
 *  dropdown — the Sync page's rule. */
const VIEWS = [
  { value: "board", label: "Board" },
  { value: "table", label: "Table" },
  { value: "backlog", label: "Backlog" },
  { value: "timeline", label: "Timeline" },
] as const satisfies ReadonlyArray<{ value: ProjectView; label: string }>;

function isView(v: string | null): v is ProjectView {
  return v === "board" || v === "table" || v === "backlog" || v === "timeline";
}

/**
 * One project: its board, its table, its backlog and its timeline.
 *
 * The chrome follows `app/src/pages/SyncPage.tsx` exactly — the tab strip
 * alone on the container's bottom rule, then a fixed `h-12` toolbar with what
 * the view is scoped to on the left and how it is going plus what you can do
 * about it on the right. The height is fixed rather than sized to its contents
 * because switching views must not jolt the work below it.
 */
export default function ProjectPage() {
  const { projectId } = useParams();
  const id = Number(projectId);
  const navigate = useNavigate();

  const project = useProjectsStore((s) => s.projects.find((p) => p.id === id) ?? null);
  const tasks = useProjectsStore((s) => s.tasks);
  const loadProjects = useProjectsStore((s) => s.loadProjects);
  const openProject = useProjectsStore((s) => s.open);
  const reload = useProjectsStore((s) => s.reload);
  const moveTask = useProjectsStore((s) => s.moveTask);
  const createTask = useProjectsStore((s) => s.createTask);
  const archiveProject = useProjectsStore((s) => s.archiveProject);

  const [listed, setListed] = useState(false);
  const [view, setView] = useState<ProjectView>(() => {
    const stored = localStorage.getItem(VIEW_KEY);
    return isView(stored) ? stored : "board";
  });

  // The timeline's axis is the page's state rather than the view's, because
  // its control lives in the toolbar below the tab strip — and it sticks, the
  // way the view above it and the week grid's `24h` toggle do.
  const [zoom, setZoom] = useState<TimelineZoom>(() => {
    const stored = localStorage.getItem(TIMELINE_ZOOM_KEY);
    return isTimelineZoom(stored) ? stored : "week";
  });

  useEffect(() => {
    localStorage.setItem(VIEW_KEY, view);
  }, [view]);

  useEffect(() => {
    localStorage.setItem(TIMELINE_ZOOM_KEY, zoom);
  }, [zoom]);

  // `status: "all"` so a project opened by its link still resolves once it has
  // been archived — the index lists the active ones, this page is a name you
  // already have.
  useEffect(() => {
    setListed(false);
    loadProjects({ status: "all" }).finally(() => setListed(true));
  }, [loadProjects, id]);

  useEffect(() => {
    if (Number.isFinite(id)) void openProject(id);
  }, [openProject, id]);

  // Someone else's write — the chat agent's, or another page's — lands as a
  // window event, the way the calendar hears CALENDAR_UPDATED_EVENT.
  useEffect(() => {
    const onUpdated = () => void reload();
    window.addEventListener(PROJECTS_UPDATED_EVENT, onUpdated);
    return () => window.removeEventListener(PROJECTS_UPDATED_EVENT, onUpdated);
  }, [reload]);

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

  return (
    <div className="flex h-full flex-col">
      {/* Tabs alone on the rule the active tab underlines. */}
      <div className="shrink-0 flex items-end border-b border-border-subtle px-5 pt-4">
        <ViewTabs tabs={VIEWS} value={view} onChange={setView} />
      </div>

      {/* Fixed-height toolbar: scope on the left, state and actions right. */}
      <div className="shrink-0 flex h-12 items-center gap-2.5 px-5">
        <span className="flex min-w-0 items-center gap-1.5 text-[11px] text-muted-foreground">
          {project.subject_code ? (
            <>
              <SubjectIcon code={project.subject_code} size={12} />
              {displayCode(project.subject_code)}
            </>
          ) : (
            "Personal"
          )}
        </span>
        <span aria-hidden className="text-border">/</span>
        {/* Not an <h1>: the tab strip already names the page, and a heading
            element here would be a second title on the same rule. */}
        <span className="min-w-0 truncate font-display text-[13px] font-semibold tracking-tight text-foreground">
          {project.name}
        </span>

        {project.status !== "active" && (
          <Badge variant="secondary" className="shrink-0 text-[11px]">
            Archived
          </Badge>
        )}

        <span className="flex-1" />

        {view === "timeline" && <TimelineZoomControl value={zoom} onChange={setZoom} />}

        {project.due_at && (
          <span className="flex shrink-0 items-center gap-1 text-[11px] text-muted-foreground">
            Due <DueChip dueAt={project.due_at} />
          </span>
        )}
        <span className="shrink-0 text-[11px] tabular-nums text-muted-foreground">
          {progress.done}/{progress.total} done
        </span>
        <Button
          variant="ghost"
          size="sm"
          className="h-7 shrink-0 text-xs text-muted-foreground hover:text-foreground"
          onClick={() => {
            // Archived is off the board rather than deleted, so the way out is
            // the list — a board whose project the list no longer carries has
            // nothing left to say.
            archiveProject(project.id)
              .then(() => navigate("/projects"))
              .catch((e) => console.error("archive failed", e));
          }}
        >
          <Archive size={13} /> Archive
        </Button>
      </div>

      <div className="min-h-0 flex-1">
        {view === "board" && (
          <ProjectBoard
            project={project}
            nodes={nodes}
            onMove={handleMove}
            onCreate={handleCreate}
          />
        )}
        {view === "table" && (
          <ProjectTable
            project={project}
            nodes={nodes}
            onMove={handleMove}
            onCreate={handleCreate}
          />
        )}
        {view === "backlog" && (
          <ProjectBacklog
            project={project}
            nodes={nodes}
            onMove={handleMove}
            onCreate={handleCreate}
          />
        )}
        {view === "timeline" && (
          <ProjectTimeline project={project} nodes={nodes} zoom={zoom} />
        )}
      </div>
    </div>
  );
}
