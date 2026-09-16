import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { Link, Navigate, useNavigate, useParams } from "react-router-dom";
import { TrashSimple } from "@phosphor-icons/react";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { InlineAdd } from "@/components/projects/InlineAdd";
import { StatusPill } from "@/components/projects/StatusPill";
import { AgentMark, DueChip, TaskGlyph } from "@/components/projects/TaskMarks";
import { DateTimeField } from "@/components/projects/DateTimeField";
import { DraftField } from "@/components/projects/DraftField";
import { projectHref } from "@/components/projects/projectHref";
import { ProjectCrumbs } from "@/components/projects/ProjectCrumbs";
import { taskHref } from "@/components/projects/taskHref";
import {
  appendSlot,
  columnOf,
  promotionTarget,
  taskTree,
} from "@/components/projects/taskTree";
import { fmtAgo, fmtClock, sqliteUtcToMs } from "@/lib/format";
import { cn } from "@/lib/utils";
import { PROJECTS_UPDATED_EVENT, type DbProject, type DbProjectTask } from "@/lib/projects";
import { useProjectsStore } from "@/stores/projectsStore";

/**
 * One task as a page of its own — a description you can write, metadata you
 * can edit, and its subtasks. The board and the table are where a plan is
 * arranged; this is where one piece of it is actually thought about, so it is
 * a full page rather than a dialog over the board it came from.
 *
 * It reads the whole project, not just the row: a status only means something
 * against the project's own columns, and `moveTask` is told a column id that
 * has to come from that board.
 */

// ── Property rows ────────────────────────────────────────────────────────────

/** Notion's property grammar: the label is furniture on the left, the value is
 *  the control on the right, and the row is the same height whether the value
 *  is a pill, a field or a sentence. */
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
  const id = Number(projectId);
  const tid = Number(taskId);
  const navigate = useNavigate();

  const project = useProjectsStore((s) => s.projects.find((p) => p.id === id) ?? null);
  const tasks = useProjectsStore((s) => s.tasks);
  const loadProjects = useProjectsStore((s) => s.loadProjects);
  const openProject = useProjectsStore((s) => s.open);
  const reload = useProjectsStore((s) => s.reload);
  const updateTask = useProjectsStore((s) => s.updateTask);
  const moveTask = useProjectsStore((s) => s.moveTask);
  const createTask = useProjectsStore((s) => s.createTask);
  const deleteTask = useProjectsStore((s) => s.deleteTask);

  const [listed, setListed] = useState(false);
  const [confirmDelete, setConfirmDelete] = useState(false);

  // `status: "all"` for the same reason `ProjectPage` does it: this is a page
  // reached by a link you already hold, and a task of an archived project is
  // still a task you can open.
  useEffect(() => {
    setListed(false);
    loadProjects({ status: "all" }).finally(() => setListed(true));
  }, [loadProjects, id]);

  useEffect(() => {
    if (Number.isFinite(id)) void openProject(id);
  }, [openProject, id]);

  // The one refresh path: a write here, a write on the board in another tab
  // and a write the chat agent made through the CLI all arrive as this event.
  useEffect(() => {
    const onUpdated = () => void reload();
    window.addEventListener(PROJECTS_UPDATED_EVENT, onUpdated);
    return () => window.removeEventListener(PROJECTS_UPDATED_EVENT, onUpdated);
  }, [reload]);

  const task = useMemo(() => tasks.find((t) => t.id === tid) ?? null, [tasks, tid]);
  const parent = useMemo(
    () => (task?.parent_id != null ? tasks.find((t) => t.id === task.parent_id) ?? null : null),
    [tasks, task?.parent_id],
  );
  const children = useMemo(
    () => (task ? tasks.filter((t) => t.parent_id === task.id) : []),
    [tasks, task],
  );
  // Only for the `moveTask` slots below — the same shaping every other view
  // reads its positions out of.
  const nodes = useMemo(() => taskTree(tasks), [tasks]);

  const patch = useCallback(
    (next: Parameters<typeof updateTask>[1]) => {
      if (!task) return;
      updateTask(task.id, next).catch((e) => console.error("update task failed", e));
    },
    [task, updateTask],
  );

  /** Every column change on this page, wherever it comes from. `moveTask` is
   *  the only writer of `column_id`, `position` and `done_at` — `updateTask`
   *  cannot touch them at all — so the status pill and the subtask checkboxes
   *  are the same call with a different destination. */
  const move = useCallback(
    (which: number, columnId: string) => {
      const slot = appendSlot(nodes, columnId);
      moveTask(which, columnId, slot.before, slot.after).catch((e) =>
        console.error("move task failed", e),
      );
    },
    [moveTask, nodes],
  );

  if (!Number.isFinite(id) || !Number.isFinite(tid)) return <Navigate to="/projects" replace />;

  if (!project || !task) {
    const back = project ? projectHref(project) : "/projects";
    return (
      <div className="flex h-full items-center justify-center px-6">
        {listed ? (
          <p className="text-xs text-muted-foreground">
            That task is gone.{" "}
            <Link to={back} className="text-brand hover:underline">
              {project ? "Back to the project" : "Back to Projects"}
            </Link>
          </p>
        ) : (
          <p className="text-xs text-muted-foreground">Loading…</p>
        )}
      </div>
    );
  }

  const createdMs = sqliteUtcToMs(task.created_at);

  return (
    <div className="page-scroll">
      <div className="mx-auto max-w-3xl px-6 py-6">
        {/* Where you are: the list, the subject, the project, then this. */}
        <nav
          aria-label="Breadcrumb"
          className="flex min-w-0 items-center gap-1.5 text-[11px] text-muted-foreground"
        >
          <ProjectCrumbs project={project} />
          <Link
            to={projectHref(project)}
            className="min-w-0 truncate transition-colors hover:text-foreground"
          >
            {project.name}
          </Link>
        </nav>

        <TaskTitle task={task} project={project} onRename={(title) => patch({ title })} />

        <div className="mt-5 flex flex-col gap-0.5 border-t border-border pt-4">
          <Row label="Status">
            <StatusPill
              project={project}
              columnId={task.column_id}
              onPick={(columnId) => move(task.id, columnId)}
            />
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
                  to={taskHref(project.id, parent)}
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
              {/* `fmtDate` is deliberately not called on `created_at` itself:
                  it takes an ISO string, and this column is SQLite's naive UTC
                  — read as local it lands hours out. `sqliteUtcToMs` is the
                  one place that conversion belongs. */}
              {createdMs ? fmtClock(createdMs, true) : "—"}
            </span>
            {createdMs != null && (
              <span className="text-[11px] text-muted-foreground/60">{fmtAgo(createdMs)}</span>
            )}
            <AgentMark source={task.source} />
          </Row>
        </div>

        <TaskBody task={task} onSave={(body) => patch({ body })} />

        {/* Subtasks are one level deep (`app/src/lib/projects.ts`), so a
            subtask has no list of its own to draw. It is not silently
            missing: the Parent row above says what this row is, which is the
            whole reason there is nothing here. */}
        {task.parent_id == null && (
          <Subtasks
            project={project}
            subtasks={children}
            onToggle={(child, columnId) => move(child.id, columnId)}
            onAdd={(title) =>
              createTask({ projectId: project.id, parentId: task.id, title }).catch((e) =>
                console.error("create subtask failed", e),
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

      <Dialog open={confirmDelete} onOpenChange={setConfirmDelete}>
        <DialogContent className="sm:max-w-sm" showCloseButton={false}>
          <DialogHeader>
            <DialogTitle>Delete this task?</DialogTitle>
            <DialogDescription>
              {/* Said out loud because the cascade is in the migration, not on
                  screen: nothing upstream has a copy of any of this, so there
                  is nothing to sync it back from. */}
              “{task.title}” will be removed from {project.name}
              {children.length > 0
                ? `, and so will its ${children.length} ${
                    children.length === 1 ? "subtask" : "subtasks"
                  }.`
                : "."}{" "}
              This cannot be undone.
            </DialogDescription>
          </DialogHeader>
          <DialogFooter>
            <Button variant="outline" onClick={() => setConfirmDelete(false)}>
              Keep it
            </Button>
            <Button
              variant="destructive"
              onClick={() => {
                setConfirmDelete(false);
                deleteTask(task.id)
                  .then(() => navigate(projectHref(project)))
                  .catch((e) => console.error("delete task failed", e));
              }}
            >
              Delete
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </div>
  );
}

/**
 * The title, editable where it is drawn.
 *
 * Empty reverts rather than commits — a nameless task is one you can no longer
 * find on any of the four views — and so does an unchanged one, which keeps a
 * stray click off the write path entirely.
 */
function TaskTitle({
  task,
  project,
  onRename,
}: {
  task: DbProjectTask;
  project: DbProject;
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
    // The tab strip titles a tab from its path alone (`taskHref`), so a rename
    // that only wrote the row would leave the tab you are looking at wearing
    // the old title until it was reopened. Replacing the entry rather than
    // pushing keeps the back arrow pointing where it did.
    navigate(taskHref(project.id, { id: task.id, title }), { replace: true });
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
 * The description.
 *
 * Plain text, deliberately: a task body is a paragraph and a couple of
 * reminders, and wiring the markdown renderer in would mean a read mode and an
 * edit mode for something you mostly write two lines into. `whitespace-pre-wrap`
 * is therefore the whole renderer.
 *
 * The draft follows the row only when the row changes, for the reason
 * `DraftField` gives: every write in the app fires `PROJECTS_UPDATED_EVENT`,
 * and a draft that re-synced on each one would overwrite what is being typed.
 */
function TaskBody({
  task,
  onSave,
}: {
  task: DbProjectTask;
  onSave: (body: string | null) => void;
}) {
  const [draft, setDraft] = useState(task.body ?? "");
  useEffect(() => setDraft(task.body ?? ""), [task.id, task.body]);

  const save = () => {
    const body = draft.trim();
    if (body === (task.body ?? "")) return;
    onSave(body || null);
  };

  return (
    <div className="mt-6">
      <textarea
        value={draft}
        placeholder="Write what this actually involves…"
        onChange={(e) => setDraft(e.target.value)}
        onBlur={save}
        onKeyDown={(e) => {
          // ⌘↵ saves without leaving the field — the composer's gesture, and
          // the only way to save a body you are not finished with.
          if (e.key === "Enter" && (e.metaKey || e.ctrlKey)) {
            e.preventDefault();
            save();
          }
        }}
        className={cn(
          "field-sizing-content min-h-24 w-full resize-none whitespace-pre-wrap rounded-lg border border-transparent",
          "bg-transparent px-2 py-1.5 text-[13px] leading-relaxed text-foreground outline-none transition-colors",
          "hover:border-border-subtle focus:border-brand/40 focus:bg-card",
          "placeholder:text-muted-foreground/60",
        )}
      />
    </div>
  );
}

/**
 * The task's own children.
 *
 * Ticking one off is a `moveTask` between the board's done column and the
 * first column that is work — the same move the board makes by drag and the
 * table by pill, so `done_at` and the column it sits in can never disagree.
 * A board with no such column to move to leaves the control dead and says why,
 * rather than dropping a click on the floor.
 */
function Subtasks({
  project,
  subtasks,
  onToggle,
  onAdd,
}: {
  project: DbProject;
  /** Not named `children`: that prop is JSX's, and a component taking one by
   *  that name is a component whose contents anyone can accidentally replace. */
  subtasks: DbProjectTask[];
  onToggle: (child: DbProjectTask, columnId: string) => void;
  onAdd: (title: string) => void;
}) {
  const doneColumn = project.columns.find((c) => c.kind === "done") ?? null;
  const activeColumn = promotionTarget(project);

  /** Where a tick would send this subtask, or null when the board has nowhere
   *  to send it. */
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

      <div className="mt-2 divide-y divide-border-subtle overflow-hidden rounded-lg border border-border">
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
                  "shrink-0 rounded-full p-0.5 transition-opacity",
                  target == null ? "cursor-not-allowed opacity-30" : "cursor-pointer hover:opacity-70",
                )}
              >
                <TaskGlyph kind={columnOf(project, child.column_id)?.kind ?? null} />
              </button>

              <Link
                to={taskHref(project.id, child)}
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
            // No column of its own: `createTask` files a subtask in its
            // parent's column, which is the one that keeps it out of a Backlog
            // view that only lists top-level rows.
            onAdd={onAdd}
          />
        </div>
      </div>
    </div>
  );
}
