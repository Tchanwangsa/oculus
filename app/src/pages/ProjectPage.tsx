import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { Link, Navigate, useNavigate, useParams } from "react-router-dom";
import { Badge } from "@/components/ui/badge";
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
import { cn } from "@/lib/utils";
import { PROJECTS_UPDATED_EVENT, type DbProject } from "@/lib/projects";
import { useProjectsStore } from "@/stores/projectsStore";

/**
 * The two halves of a project: what it *is*, and what is left to do about it.
 *
 * They are the top strip because they are not two views of one thing — the
 * Overview answers "what was this again", the Tasks tab answers "what now" —
 * and putting the three task views up here beside them was what made the
 * strip read as four unrelated buttons.
 */
type ProjectTab = "overview" | "tasks";

/** The ways to read the task list. Board, table, timeline — three shapes of
 *  the same rows, which is exactly what a subordinate strip is for. */
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

/**
 * `VIEW_KEY` used to hold a fourth value, `"backlog"`, and that string is
 * sitting in the localStorage of anyone who used the Backlog view before it
 * was folded into the board's backlog column. It has to resolve to something:
 * an unrecognised stored view would leave the Tasks tab rendering nothing at
 * all, which is a blank page on the machine of the one person who used the
 * feature most. Board is where a backlog stub lives now, so it is the landing.
 */
function isView(v: string | null): v is TaskView {
  return v === "board" || v === "table" || v === "timeline";
}

/**
 * One project: its Overview, and its tasks read three ways.
 *
 * The chrome follows `app/src/pages/SyncPage.tsx` — the tab strip alone on the
 * container's bottom rule, then a fixed `h-12` toolbar with what the page is
 * scoped to on the left and how it is going plus what you can do about it on
 * the right. The height is fixed rather than sized to its contents because
 * switching views must not jolt the work below.
 *
 * Under it, on Tasks only, a second row carries the three view tabs. That row
 * appearing does move the seam between Overview and Tasks — but those two are
 * a scrolling document and a board, with nothing in common to jolt. What the
 * fixed height was actually protecting is switching *between* the task views,
 * and that is untouched: all three sit under both rows at the same height.
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
  const updateProject = useProjectsStore((s) => s.updateProject);
  const actions = useProjectActions();

  const [listed, setListed] = useState(false);

  // Overview is the default: a project you have not opened before is one you
  // are least likely to remember the shape of.
  const [tab, setTab] = useState<ProjectTab>(() => {
    const stored = localStorage.getItem(TAB_KEY);
    return isTab(stored) ? stored : "overview";
  });
  const [view, setView] = useState<TaskView>(() => {
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
    localStorage.setItem(TAB_KEY, tab);
  }, [tab]);

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

  /** Every field the Overview edits. One call rather than a handler each: the
   *  patch shape is already the lib's, and the page has nothing to add to any
   *  of them beyond logging the failure. */
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

  return (
    <div className="flex h-full flex-col">
      {/* Tabs alone on the rule the active tab underlines. */}
      <div className="shrink-0 flex items-end border-b border-border-subtle px-5 pt-4">
        <ViewTabs tabs={TABS} value={tab} onChange={setTab} />
      </div>

      {/* Fixed-height toolbar: scope on the left, state and actions right. */}
      <div className="shrink-0 flex h-12 items-center gap-2.5 px-5">
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

        <span className="flex-1" />

        {project.due_at && (
          <span className="flex shrink-0 items-center gap-1 text-[11px] text-muted-foreground">
            Due <DueChip dueAt={project.due_at} />
          </span>
        )}
        {/* Not on Overview: its Properties list says the same fraction a few
            pixels below, with a bar and a percentage. Hiding a text span does
            not move the seam the fixed height exists to hold still. */}
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
            // Archived is off the board rather than deleted, so the way out is
            // the list — a board whose project the list no longer carries has
            // nothing left to say. A delete leaves even less, so it goes the
            // same way; unarchiving leaves you exactly where you were.
            actions.onArchive(project);
            navigate("/projects");
          }}
          onUnarchive={() => actions.onUnarchive(project)}
          onDelete={() => {
            actions.onDelete(project);
            navigate("/projects");
          }}
        />
      </div>

      {/* The task views get a row to themselves rather than riding along after
          the project's name: sharing that row made them read as two more items
          in the breadcrumb, and a long project name pushed them around.
          The timeline's zoom comes with them — it is scoped to one of these
          views, so leaving it up in the toolbar would split a control from the
          thing it controls across two rows. */}
      {tab === "tasks" && (
        <div className="shrink-0 flex h-9 items-center gap-2.5 px-5">
          <TaskViewTabs value={view} onChange={setView} />
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
 * Board / Table / Timeline, nested under Tasks.
 *
 * Tabs rather than a dropdown — they are still three peers — but deliberately
 * not a second `ViewTabs`: two identical underline strips stacked would read
 * as two levels of the same rank and fight each other for the indigo rule. So
 * this one is a step quieter in every dimension it can be: smaller, no
 * underline, and the active one marked by a fill rather than a colour.
 *
 * It sits on a row of its own rather than sharing the toolbar with the
 * project's name, which is where it started: next to a breadcrumb it read as
 * two more crumbs, and a long project name shoved it along the row.
 */
function TaskViewTabs({
  value,
  onChange,
  className,
}: {
  value: TaskView;
  onChange: (value: TaskView) => void;
  className?: string;
}) {
  return (
    <div role="tablist" className={cn("flex items-center gap-0.5", className)}>
      {TASK_VIEWS.map((v) => {
        const active = v.value === value;
        return (
          <button
            key={v.value}
            type="button"
            role="tab"
            aria-selected={active}
            onClick={() => onChange(v.value)}
            className={cn(
              "cursor-pointer rounded-full px-2 py-0.5 text-[11.5px] font-medium transition-colors",
              active
                ? "bg-secondary text-foreground"
                : "text-muted-foreground hover:text-foreground",
            )}
          >
            {v.label}
          </button>
        );
      })}
    </div>
  );
}

/**
 * The project's name, editable where it is drawn.
 *
 * `ProjectMenu` carries a rename dialog too, and both are wanted: the dialog
 * is how you rename a project from a list row, where the row is a link and
 * turning it into a field would stop it being one. This is how you rename the
 * thing you are already looking at.
 *
 * Empty reverts rather than commits — a nameless project is unfindable in
 * every list it appears in — and so does an unchanged one, which keeps a stray
 * click off the write path entirely.
 */
function ProjectTitle({
  project,
  onRename,
}: {
  project: DbProject;
  onRename: (name: string) => void;
}) {
  const navigate = useNavigate();
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
    // `tabInfo` titles a tab from its path alone and has no project list to
    // look a name up in — it reads the `?n=` `projectHref` put there. So a
    // rename that only wrote the row would leave the tab you are looking at
    // wearing the old name until it was reopened. Replacing the entry rather
    // than pushing keeps the back arrow pointing where it did.
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
    // Not an <h1>: the tab strip already names the page, and a heading element
    // here would be a second title on the same rule.
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
      className={cn(shared, "cursor-text truncate rounded-md outline-none")}
    >
      {project.name}
    </span>
  );
}
