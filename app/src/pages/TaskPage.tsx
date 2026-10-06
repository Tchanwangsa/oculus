import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { Link, Navigate, useNavigate, useParams } from "react-router-dom";
import { TrashSimple } from "@phosphor-icons/react";
import { Button } from "@/components/ui/button";
import { ConfirmDialog } from "@/components/ui/ConfirmDialog";
import { InlineAdd } from "@/components/projects/InlineAdd";
import { StatusPill } from "@/components/projects/StatusPill";
import { AgentMark, DueChip, TaskGlyph } from "@/components/projects/TaskMarks";
import { DateTimeField } from "@/components/projects/DateTimeField";
import { DraftField } from "@/components/projects/DraftField";
import { projectHref } from "@/components/projects/projectHref";
import { ProjectCrumbs } from "@/components/projects/ProjectCrumbs";
import { ProjectPicker } from "@/components/projects/ProjectPicker";
import { PaneHeaderRow, PaneTrail, useInSidePanel } from "@/components/tabs/PaneHeader";
import { NoteField } from "@/components/documents/NoteField";
import {
  attachmentPath,
  attachmentSrc,
  pendingFromFile,
  pendingFromPath,
  releaseAttachment,
  writeAttachment,
} from "@/lib/attachments";
import { useDataDir } from "@/hooks/useDataDir";
import { libraryImageSrc } from "@/lib/libraryLinks";
import { navigateActive } from "@/lib/tabRouters";
import { taskHref } from "@/components/projects/taskHref";
import {
  appendSlot,
  columnOf,
  promotionTarget,
  taskTree,
} from "@/components/projects/taskTree";
import { fmtAgo, fmtClock, sqliteUtcToMs } from "@/lib/format";
import { cn } from "@/lib/utils";
import { useTaskList } from "@/hooks/useTaskList";
import {
  boardOf,
  PROJECTS_UPDATED_EVENT,
  taskBodyEdit,
  type DbProject,
  type DbProjectTask,
} from "@/lib/projects";
import { useProjectsStore } from "@/stores/projectsStore";
import { useWindowEvent } from "@/hooks/useEvents";
import { ListCard } from "@/components/ui/PageParts";

/**
 * One task as a page of its own: an editable description, metadata and its
 * subtasks. It reads the whole project, because a status only means something
 * against the project's own columns and `moveTask` needs a column id from it.
 *
 * Two routes, one page: a filed task is `/projects/:projectId/tasks/:taskId`,
 * an unfiled one (no project at all) is `/tasks/:taskId`. `project` is
 * therefore nullable and the board is read through `boardOf`, which gives the
 * default four columns when there is none. The store holds one open project's
 * tasks, so the unfiled half reads its rows through `useTaskList` instead.
 *
 * The Project row is a picker over `refileTask`, and a refile re-navigates to
 * the task's new href, since that href is built from the project.
 */

// ── Property rows ────────────────────────────────────────────────────────────

/** A property row: label on the left, control on the right, fixed height. */
function Row({ label, children }: { label: string; children: React.ReactNode }) {
  return (
    <div className="flex min-h-8 items-center gap-3">
      <span className="w-24 shrink-0 text-[11px] text-muted-foreground">{label}</span>
      <div className="flex min-w-0 flex-1 items-center gap-2">{children}</div>
    </div>
  );
}

// ── The page ─────────────────────────────────────────────────────────────────

export default function TaskPage() {
  const { projectId, taskId } = useParams();
  // No project segment at all (not an unparseable one) means an unfiled task.
  const filed = projectId !== undefined;
  const id = Number(projectId);
  const tid = Number(taskId);
  const navigate = useNavigate();
  const inSide = useInSidePanel();

  const project = useProjectsStore((s) =>
    filed ? s.projects.find((p) => p.id === id) ?? null : null,
  );
  const storeProjects = useProjectsStore((s) => s.projects);
  const storeTasks = useProjectsStore((s) => s.tasks);
  const loadProjects = useProjectsStore((s) => s.loadProjects);
  const openProject = useProjectsStore((s) => s.open);
  const reload = useProjectsStore((s) => s.reload);
  const updateTask = useProjectsStore((s) => s.updateTask);
  const moveTask = useProjectsStore((s) => s.moveTask);
  const createTask = useProjectsStore((s) => s.createTask);
  const deleteTask = useProjectsStore((s) => s.deleteTask);
  const refileTask = useProjectsStore((s) => s.refileTask);

  // `null` reads nothing, so a filed task's page skips this query.
  const unfiled = useTaskList(filed ? null : "unfiled");
  const tasks = filed ? storeTasks : unfiled.tasks;
  // For the Project picker; both halves already hold the list at `status: "all"`.
  const projects = filed ? storeProjects : unfiled.projects;

  const [listed, setListed] = useState(false);
  const [confirmDelete, setConfirmDelete] = useState(false);
  /** Whether the read that would have found this task has landed ("gone" vs
   *  "not read yet"). */
  const ready = filed ? listed : unfiled.loaded;

  // `status: "all"`: a task of an archived project is still one you can open.
  useEffect(() => {
    if (!filed) return;
    setListed(false);
    loadProjects({ status: "all" }).finally(() => setListed(true));
  }, [loadProjects, id, filed]);

  useEffect(() => {
    if (filed && Number.isFinite(id)) void openProject(id);
  }, [openProject, id, filed]);

  // Every write (here, another tab, the agent via the CLI) arrives as this
  // event. The unfiled half has its own listener inside `useTaskList`.
  useWindowEvent(PROJECTS_UPDATED_EVENT, () => {
    if (filed) void reload();
  });

  const task = useMemo(() => tasks.find((t) => t.id === tid) ?? null, [tasks, tid]);
  const parent = useMemo(
    () => (task?.parent_id != null ? tasks.find((t) => t.id === task.parent_id) ?? null : null),
    [tasks, task?.parent_id],
  );
  const children = useMemo(
    () => (task ? tasks.filter((t) => t.parent_id === task.id) : []),
    [tasks, task],
  );
  // Only for the `moveTask` slots below.
  const nodes = useMemo(() => taskTree(tasks), [tasks]);

  const patch = useCallback(
    (next: Parameters<typeof updateTask>[1]) => {
      if (!task) return;
      updateTask(task.id, next).catch((e) => console.error("update task failed", e));
    },
    [task, updateTask],
  );

  /** Every column change on this page. `moveTask` is the only writer of
   *  `column_id`, `position` and `done_at`, so the status pill and the subtask
   *  checkboxes are the same call. */
  const move = useCallback(
    (which: number, columnId: string) => {
      const slot = appendSlot(nodes, columnId);
      moveTask(which, columnId, slot.before, slot.after).catch((e) =>
        console.error("move task failed", e),
      );
    },
    [moveTask, nodes],
  );

  /** Refile, then replace the route: a task's href is built from its project
   *  (`taskHref`), so the old path would read as "that task is gone". */
  const refile = useCallback(
    (projectId: number | null) => {
      if (!task) return;
      refileTask(task.id, projectId)
        .then(() => navigate(taskHref(projectId, task), { replace: true }))
        .catch((e) => console.error("refile task failed", e));
    },
    [navigate, refileTask, task],
  );

  if (!Number.isFinite(tid) || (filed && !Number.isFinite(id))) {
    return <Navigate to={filed ? "/projects" : "/tasks"} replace />;
  }

  if ((filed && !project) || !task) {
    const back = project ? projectHref(project) : filed ? "/projects" : "/tasks";
    return (
      <div className="flex h-full items-center justify-center px-6">
        {ready ? (
          <p className="text-xs text-muted-foreground">
            That task is gone.{" "}
            <Link to={back} className="text-brand hover:underline">
              {project ? "Back to the project" : filed ? "Back to Projects" : "Back to Tasks"}
            </Link>
          </p>
        ) : (
          <p className="text-xs text-muted-foreground">Loading…</p>
        )}
      </div>
    );
  }

  const createdMs = sqliteUtcToMs(task.created_at);

  const crumbs = (
    <nav
      aria-label="Breadcrumb"
      className="flex min-w-0 items-center gap-1.5 text-[11px] text-muted-foreground"
    >
      {/* An unfiled task's trail is just the Tasks page. */}
      {project ? (
        <>
          <ProjectCrumbs project={project} foldSubject />
          <button
            type="button"
            data-tab-href={projectHref(project)}
            onClick={() => navigateActive(projectHref(project))}
            className="min-w-0 cursor-pointer truncate transition-colors hover:text-foreground"
          >
            {project.name}
          </button>
        </>
      ) : (
        <button
          type="button"
          data-tab-href="/tasks"
          onClick={() => navigateActive("/tasks")}
          className="shrink-0 cursor-pointer transition-colors hover:text-foreground"
        >
          Tasks
        </button>
      )}
    </nav>
  );

  const page = (
    <div className={inSide ? "page-scroll min-h-0 flex-1" : "page-scroll"}>
      <div className="mx-auto max-w-3xl px-6 py-6">
        {!inSide && crumbs}

        <TaskTitle task={task} onRename={(title) => patch({ title })} />

        <div className="mt-5 flex flex-col gap-0.5 border-t border-border pt-4">
          <Row label="Status">
            <StatusPill
              project={project}
              columnId={task.column_id}
              onPick={(columnId) => move(task.id, columnId)}
            />
          </Row>

          <Row label="Project">
            {/* `refileTask` refuses to move a subtask on its own, so for one
                this is a readout, not a picker. */}
            {task.parent_id != null ? (
              <span className="flex min-w-0 items-center gap-1.5 text-xs">
                <span className={project ? "text-foreground" : "text-muted-foreground"}>
                  {project?.name ?? "Unfiled"}
                </span>
                <span className="text-[11px] text-muted-foreground/70">
                  — follows its parent
                </span>
              </span>
            ) : (
              <ProjectPicker
                projects={projects}
                value={task.project_id}
                label="Move to"
                unfiledHint="Take it out of every project"
                onPick={refile}
              >
                <button
                  type="button"
                  aria-label="Change project"
                  className={cn(
                    "-ml-1.5 min-w-0 cursor-pointer truncate rounded-md px-1.5 py-0.5 text-left text-xs transition-colors hover:bg-accent",
                    project ? "text-foreground" : "text-muted-foreground",
                  )}
                >
                  {project?.name ?? "Unfiled"}
                </button>
              </ProjectPicker>
            )}
          </Row>

          <Row label="Due">
            <DateTimeField
              value={task.due_at}
              defaultTime="end"
              onCommit={(dueAt) => patch({ dueAt })}
            />
          </Row>

          <Row label="Starts">
            <DateTimeField
              value={task.starts_at}
              defaultTime="start"
              onCommit={(startsAt) => patch({ startsAt })}
            />
          </Row>

          <Row label="Estimate">
            <DraftField
              placeholder="—"
              value={task.estimate_minutes == null ? "" : String(task.estimate_minutes)}
              onCommit={(next) => {
                const n = Number(next.trim());
                patch({
                  estimateMinutes: next.trim() === "" || !Number.isFinite(n) ? null : Math.round(n),
                });
              }}
              className="w-20"
            />
            <span className="text-[11px] text-muted-foreground">minutes</span>
          </Row>

          <Row label="Parent">
            {parent ? (
              <span className="flex min-w-0 items-center gap-1.5 text-xs text-muted-foreground">
                <span>Subtask of</span>
                <Link
                  to={taskHref(task.project_id, parent)}
                  className="min-w-0 truncate text-foreground hover:underline"
                >
                  {parent.title}
                </Link>
              </span>
            ) : children.length > 0 ? (
              <span className="text-xs text-muted-foreground">
                <span className="text-foreground">{children.length}</span>{" "}
                {children.length === 1 ? "subtask" : "subtasks"}
              </span>
            ) : (
              <span className="text-xs text-muted-foreground">Top-level task</span>
            )}
          </Row>

          <Row label="Added">
            <span className="text-xs text-muted-foreground">
              {/* Not `fmtDate(created_at)`: the column is SQLite's naive UTC. */}
              {createdMs ? fmtClock(createdMs, true) : "—"}
            </span>
            {createdMs != null && (
              <span className="text-[11px] text-muted-foreground/60">{fmtAgo(createdMs)}</span>
            )}
            <AgentMark source={task.source} />
          </Row>
        </div>

        <TaskBody
          key={task.id}
          task={task}
          project={project}
          onSave={(body) => patch({ body })}
        />

        {/* Subtasks are one level deep, so a subtask has no list. */}
        {task.parent_id == null && (
          <Subtasks
            project={project}
            subtasks={children}
            onToggle={(child, columnId) => move(child.id, columnId)}
            onAdd={(title) =>
              // In the parent's project, even when that is none.
              createTask({ projectId: task.project_id, parentId: task.id, title }).catch(
                (e) => console.error("create subtask failed", e),
              )
            }
          />
        )}

        <div className="mt-10 border-t border-border pt-4">
          <Button
            variant="ghost"
            size="sm"
            className="h-7 text-xs text-muted-foreground hover:bg-destructive/10 hover:text-destructive"
            onClick={() => setConfirmDelete(true)}
          >
            <TrashSimple size={13} /> Delete task
          </Button>
        </div>
      </div>

      <ConfirmDialog
        open={confirmDelete}
        onCancel={() => setConfirmDelete(false)}
        title="Delete this task?"
        description={
          <>
            “{task.title}” will be removed{project ? ` from ${project.name}` : ""}
            {children.length > 0
              ? `, and so will its ${children.length} ${
                  children.length === 1 ? "subtask" : "subtasks"
                }.`
              : "."}{" "}
            This cannot be undone.
          </>
        }
        confirmLabel="Delete"
        onConfirm={() => {
          setConfirmDelete(false);
          deleteTask(task.id)
            .then(() => navigate(project ? projectHref(project) : "/tasks"))
            .catch((e) => console.error("delete task failed", e));
        }}
      />
    </div>
  );

  // In the side panel the trail leaves the scroll for a row of its own,
  // which doubles as the panel's header; the title below stays the leaf.
  if (!inSide) return page;
  return (
    <div className="flex h-full flex-col">
      <PaneHeaderRow className="flex h-11 shrink-0 items-center gap-2.5 border-b border-border-subtle px-5">
        <PaneTrail>{crumbs}</PaneTrail>
      </PaneHeaderRow>
      {page}
    </div>
  );
}

/** The title, editable in place. Empty or unchanged reverts rather than
 *  commits: a nameless task cannot be found on any view. */
function TaskTitle({
  task,
  onRename,
}: {
  task: DbProjectTask;
  onRename: (title: string) => void;
}) {
  const navigate = useNavigate();
  const [editing, setEditing] = useState(false);
  const [draft, setDraft] = useState(task.title);
  const ref = useRef<HTMLInputElement>(null);

  useEffect(() => {
    if (editing) ref.current?.select();
  }, [editing]);

  const commit = () => {
    setEditing(false);
    const title = draft.trim();
    if (!title || title === task.title) {
      setDraft(task.title);
      return;
    }
    onRename(title);
    // The tab strip titles a tab from its path (`taskHref`), so re-navigate;
    // replace keeps the back arrow where it was.
    navigate(taskHref(task.project_id, { id: task.id, title }), { replace: true });
  };

  const edit = () => {
    setDraft(task.title);
    setEditing(true);
  };

  const shared =
    "mt-2 w-full text-[22px] font-semibold leading-tight tracking-tight outline-none";

  if (editing) {
    return (
      <input
        ref={ref}
        value={draft}
        onChange={(e) => setDraft(e.target.value)}
        onBlur={commit}
        onKeyDown={(e) => {
          if (e.key === "Enter") commit();
          if (e.key === "Escape") {
            setDraft(task.title);
            setEditing(false);
          }
        }}
        className={cn(shared, "rounded-md bg-transparent text-foreground")}
      />
    );
  }

  return (
    <h1
      tabIndex={0}
      onClick={edit}
      onKeyDown={(e) => {
        if (e.key === "Enter") {
          e.preventDefault();
          edit();
        }
      }}
      className={cn(
        shared,
        "cursor-text rounded-md",
        task.done_at ? "text-muted-foreground line-through" : "text-foreground",
      )}
    >
      {task.title}
    </h1>
  );
}

/**
 * The description: markdown in a `NoteField`, so the note editor's live
 * preview, maths, tables, shortcuts and toolbar; `@` searches the project's
 * subject or the whole library. Pictures go to `agents/attachments/` on
 * arrival. Blur and ⌘↵ save, unchanged writes nothing, empty writes `null`.
 * Keyed by task (below), so leaving a task saves to that task.
 */
function TaskBody({
  task,
  project,
  onSave,
}: {
  task: DbProjectTask;
  project: DbProject | null;
  onSave: (body: string | null) => void;
}) {
  const dataDir = useDataDir();
  /** An attachment in either spelling, else a path from the data dir. */
  const imageSrc = useCallback(
    (src: string) => {
      const picture = attachmentPath(src);
      return picture ? attachmentSrc(dataDir, picture) : libraryImageSrc(src, "", dataDir);
    },
    [dataDir],
  );

  const writePicture = useCallback(async (source: File | string) => {
    const pending = typeof source === "string" ? pendingFromPath(source) : pendingFromFile(source);
    try {
      return await writeAttachment(pending);
    } finally {
      // No preview strip here, so the preview URL is dead once written.
      releaseAttachment(pending);
    }
  }, []);

  return (
    <NoteField
      className="mt-6"
      text={task.body ?? ""}
      subjectId={project?.subject_id ?? null}
      imageSrc={imageSrc}
      writePicture={writePicture}
      pickerTitle="Add pictures to this task"
      notAPicture="Only images can go in a task body — use @ for a course file."
      placeholder="Write what this actually involves…"
      label="Description"
      onCommit={(text) => {
        const body = taskBodyEdit(text, task.body);
        if (body !== undefined) onSave(body);
      }}
    />
  );
}

/**
 * The task's children. A tick is a `moveTask` between the done column and the
 * first work column, so `done_at` and the column never disagree. A board with
 * no such column leaves the control disabled and says why.
 */
function Subtasks({
  project,
  subtasks,
  onToggle,
  onAdd,
}: {
  /** `null` on an unfiled task, whose board is `boardOf`'s default four. */
  project: DbProject | null;
  /** Not `children`, which is JSX's. */
  subtasks: DbProjectTask[];
  onToggle: (child: DbProjectTask, columnId: string) => void;
  onAdd: (title: string) => void;
}) {
  const doneColumn = boardOf(project).find((c) => c.kind === "done") ?? null;
  const activeColumn = promotionTarget(project);

  /** Where a tick would send this subtask, or null if nowhere. */
  const destination = (child: DbProjectTask) =>
    (child.done_at != null ? activeColumn : doneColumn)?.id ?? null;

  const done = subtasks.filter((c) => c.done_at != null).length;

  return (
    <div className="mt-8">
      <div className="flex items-baseline gap-2">
        <h2 className="text-[13px] font-semibold tracking-tight text-foreground">Subtasks</h2>
        {subtasks.length > 0 && (
          <span className="text-[11px] tabular-nums text-muted-foreground">
            {done}/{subtasks.length}
          </span>
        )}
      </div>

      <ListCard className="mt-2">
        {subtasks.length === 0 && (
          <p className="px-3 py-5 text-center text-xs text-muted-foreground">
            No subtasks yet.
          </p>
        )}

        {subtasks.map((child) => {
          const target = destination(child);
          return (
            <div
              key={child.id}
              className="flex items-center gap-2.5 px-3 py-2 transition-colors hover:bg-surface"
            >
              <button
                type="button"
                disabled={target == null}
                aria-label={child.done_at ? "Mark as not done" : "Mark as done"}
                title={
                  target == null
                    ? "This board has no column to move it to"
                    : child.done_at
                      ? "Mark as not done"
                      : "Mark as done"
                }
                onClick={() => target && onToggle(child, target)}
                className={cn(
                  "shrink-0 rounded-full p-0.5 transition-opacity will-change-[opacity]",
                  target == null ? "cursor-not-allowed opacity-30" : "cursor-pointer hover:opacity-70",
                )}
              >
                <TaskGlyph kind={columnOf(project, child.column_id)?.kind ?? null} />
              </button>

              <Link
                to={taskHref(child.project_id, child)}
                className={cn(
                  "min-w-0 flex-1 truncate text-xs hover:underline",
                  child.done_at ? "text-muted-foreground line-through" : "text-foreground",
                )}
              >
                {child.title}
              </Link>

              <AgentMark source={child.source} />
              <DueChip dueAt={child.due_at} />
            </div>
          );
        })}

        <div className="px-1.5 py-1">
          <InlineAdd
            label="New subtask"
            placeholder="One piece of this"
            // `createTask` files a subtask in its parent's column.
            onAdd={onAdd}
          />
        </div>
      </ListCard>
    </div>
  );
}
