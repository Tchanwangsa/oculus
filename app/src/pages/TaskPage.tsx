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
import { ProjectPicker } from "@/components/projects/ProjectPicker";
import { MentionInput, type MentionInputHandle } from "@/components/harness/MentionInput";
import { MentionMenu } from "@/components/harness/MentionMenu";
import { useMentionMenu } from "@/components/harness/useMentionMenu";
import { CompactMd } from "@/components/markdown/MdComponents";
import {
  imagePaths,
  pendingFromFile,
  pendingFromPath,
  releaseAttachment,
  writeAttachment,
  type PendingAttachment,
} from "@/lib/attachments";
import { useFileDrop } from "@/hooks/useFileDrop";
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
  type DbProject,
  type DbProjectTask,
} from "@/lib/projects";
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
 *
 * **Two routes, one page.** A filed task is `/projects/:projectId/tasks/:taskId`
 * and an unfiled one — a task that belongs to no project at all (migration 37)
 * — is `/tasks/:taskId`, under the universal Tasks page, because there is no
 * project segment to nest it under and inventing an "Inbox" project to have
 * one is the decision that route replaces. Everything below therefore treats
 * `project` as nullable and reads the board through `boardOf`, which hands
 * back the default four columns for a task with no project — the same board
 * `moveTask` will check a pick against. The rows come from a different place
 * in each case, and that is the only other difference: the store holds one
 * open project's tasks, so the unfiled half reads `getUnfiledTasks` through
 * `useTaskList` instead. A subtask sits in its parent's project, so that one
 * query holds this task's parent and children too.
 *
 * **It is also the door between the two.** The Project property row is a
 * picker over `refileTask`, so a task written down with nowhere to go gets
 * filed from the page where you finally decide — and the page then re-navigates
 * to its own new href, because that href is built from the project it just
 * changed.
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
  // `/tasks/:taskId` has no project segment at all — that is what says this is
  // an unfiled task, rather than a project id that failed to parse.
  const filed = projectId !== undefined;
  const id = Number(projectId);
  const tid = Number(taskId);
  const navigate = useNavigate();

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

  // The unfiled half's rows and its own refresh. `null` reads nothing, so a
  // filed task's page does not issue this query at all.
  const unfiled = useTaskList(filed ? null : "unfiled");
  const tasks = filed ? storeTasks : unfiled.tasks;
  // Every project, for the Project row's picker. Both halves already hold the
  // list at `status: "all"` — the store's for a filed task, the hook's for an
  // unfiled one — so refiling costs no read of its own.
  const projects = filed ? storeProjects : unfiled.projects;

  const [listed, setListed] = useState(false);
  const [confirmDelete, setConfirmDelete] = useState(false);
  /** Whether the read that would have found this task has landed — the
   *  difference between "gone" and "not read yet". */
  const ready = filed ? listed : unfiled.loaded;

  // `status: "all"` for the same reason `ProjectPage` does it: this is a page
  // reached by a link you already hold, and a task of an archived project is
  // still a task you can open.
  useEffect(() => {
    if (!filed) return;
    setListed(false);
    loadProjects({ status: "all" }).finally(() => setListed(true));
  }, [loadProjects, id, filed]);

  useEffect(() => {
    if (filed && Number.isFinite(id)) void openProject(id);
  }, [openProject, id, filed]);

  // The one refresh path: a write here, a write on the board in another tab
  // and a write the chat agent made through the CLI all arrive as this event.
  useEffect(() => {
    // The unfiled half has its own listener inside `useTaskList`.
    if (!filed) return;
    const onUpdated = () => void reload();
    window.addEventListener(PROJECTS_UPDATED_EVENT, onUpdated);
    return () => window.removeEventListener(PROJECTS_UPDATED_EVENT, onUpdated);
  }, [reload, filed]);

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

  /**
   * Filing this task under another project, or out of every project.
   *
   * The write is `refileTask`'s — column mapped across by kind, subtasks
   * carried along, appended at the destination's end. What belongs *here* is
   * the navigation: a task's route is built from its project (`taskHref`), so
   * a refile that only wrote the row would leave this page on a path that no
   * longer describes it — `/projects/3/tasks/7` for a task project 3 no longer
   * has, which reads as "that task is gone". Replacing rather than pushing
   * keeps the back arrow pointing where it did, exactly as a rename does.
   */
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

  return (
    <div className="page-scroll">
      <div className="mx-auto max-w-3xl px-6 py-6">
        {/* Where you are: the list, the subject, the project, then this. */}
        <nav
          aria-label="Breadcrumb"
          className="flex min-w-0 items-center gap-1.5 text-[11px] text-muted-foreground"
        >
          {/* An unfiled task's trail is one segment, because it has one list
              to have come from: the universal Tasks page. `ProjectCrumbs`
              cannot draw it — there is no project to say where it sits — and a
              crumb reading "Personal" or a dash would claim a project it does
              not have. */}
          {project ? (
            <>
              <ProjectCrumbs project={project} />
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
            {/* A subtask sits in its parent's project and `refileTask` refuses
                to move one on its own, so this cell is a readout rather than a
                picker — offering the pick would be offering a refusal. Refile
                the parent and this row follows. */}
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

        <TaskBody task={task} project={project} onSave={(body) => patch({ body })} />

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
              // A subtask sits in its parent's project — including when that
              // is no project at all, which `createTask` checks rather than
              // takes on trust (`assertCanParent`).
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

      <Dialog open={confirmDelete} onOpenChange={setConfirmDelete}>
        <DialogContent className="sm:max-w-sm" showCloseButton={false}>
          <DialogHeader>
            <DialogTitle>Delete this task?</DialogTitle>
            <DialogDescription>
              {/* Said out loud because the cascade is in the migration, not on
                  screen: nothing upstream has a copy of any of this, so there
                  is nothing to sync it back from. */}
              {/* An unfiled task is removed from nowhere in particular, so it
                  is removed full stop — naming a project it does not belong to
                  would be the one wrong sentence on a screen about a delete
                  that cannot be undone. */}
              “{task.title}” will be removed{project ? ` from ${project.name}` : ""}
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
                  .then(() => navigate(project ? projectHref(project) : "/tasks"))
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
    // The tab strip titles a tab from its path alone (`taskHref`), so a rename
    // that only wrote the row would leave the tab you are looking at wearing
    // the old title until it was reopened. Replacing the entry rather than
    // pushing keeps the back arrow pointing where it did.
    // `task.project_id` rather than a project prop: it is the same `null` an
    // unfiled task's route is built from, and the row already carries it.
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
 * The description, in markdown.
 *
 * It was a plain `<textarea>` on the argument that a task body is a paragraph
 * and a couple of reminders. It is not: a body is where the plan for one piece
 * of work is actually written, so it holds a checklist, a heading, a formula
 * and — the thing the textarea could not hold at all — the files the work is
 * *about*. So it is two modes now, the grammar `TaskTitle` above uses: what is
 * written is rendered, and clicking it puts you in the editor.
 *
 * The renderer is `CompactMd` — the chat timeline's own, not a second one.
 * That is what makes a mention free: a backticked library path is already
 * drawn as a clickable `FileChip` by `MD_COMPONENTS.code`, and a picture by
 * `MD_COMPONENTS.img`, so `@` and a pasted screenshot needed an *editor* here
 * and no rendering code whatsoever.
 *
 * The editor is the composer's box (`MentionInput`) over the shared `@` menu
 * (`useMentionMenu`), scoped to this task's project's subject — or the whole
 * library for a project with no subject and for an unfiled task, which has no
 * project at all.
 *
 * Saving is unchanged from the textarea: ⌘↵ writes without leaving the field,
 * blur writes, an unchanged body writes nothing, and an empty one writes
 * `null`. The draft follows the row only when the row changes, for the reason
 * `DraftField` gives: every write in the app fires `PROJECTS_UPDATED_EVENT`,
 * and a draft that re-synced on each one would overwrite what is being typed.
 * The editor is stronger still — it reads `initialText` once at mount and the
 * DOM is the truth after that.
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
  const [editing, setEditing] = useState(false);
  const [draft, setDraft] = useState(task.body ?? "");
  useEffect(() => setDraft(task.body ?? ""), [task.id, task.body]);
  // Another task under the same route is another body: the editor holds its
  // text in its own DOM, so it has to be closed rather than re-pointed.
  useEffect(() => setEditing(false), [task.id]);

  /** Why a picture could not be written — a refusal from Rust, or a drop of
   *  something that is not a picture. Never a reason to lose what was typed. */
  const [attachError, setAttachError] = useState<string | null>(null);
  const input = useRef<MentionInputHandle>(null);
  /** `@`, scoped to the project's subject where there is one. An unfiled task
   *  has no project, and a personal project no subject; both mean the whole
   *  library, which is the general thread's scope in the composer too. */
  const mentions = useMentionMenu({ subjectId: project?.subject_id ?? null, input });
  /** The body's whole area: the drop target for a picture, and what `leave`
   *  asks about the focus. The `@` menu used to be positioned against it and
   *  is not any more — it hangs off the caret — so the ref is this
   *  component's own. */
  const wrapRef = useRef<HTMLDivElement>(null);

  const save = () => {
    const body = draft.trim();
    if (body === (task.body ?? "")) return;
    onSave(body || null);
  };

  /**
   * Leaving the field — decided a tick later, on purpose.
   *
   * Picking a mention remounts the editor's box (`MentionInput` bumps its
   * `key` on every structural change) and a focused node that is removed
   * raises a blur, even though `commit` puts the focus straight back. Asking
   * where the focus actually *is* on the next tick is what tells that apart
   * from a click somewhere else on the page.
   */
  const leave = () => {
    mentions.close();
    window.setTimeout(() => {
      if (wrapRef.current?.contains(document.activeElement)) return;
      save();
      setEditing(false);
    }, 0);
  };

  /**
   * A picture, written the moment it arrives and inserted at the caret.
   *
   * The composer deliberately writes nothing until send, because a message may
   * never be sent. A body has no send: the picture has to be *in* the text
   * while you are still writing around it, so there is no later moment to
   * defer the write to. The cost is a file left in `agents/attachments/` if
   * the picture is then deleted from the text, and that is the accepted trade
   * — an image tag pointing at nothing would be the alternative.
   *
   * It goes in as a markdown image rather than the composer's fenced path,
   * because this text is *read* as markdown; the path itself is the same
   * data-dir-relative shape the composer writes, which is what
   * `attachmentPath` matches and `MD_COMPONENTS.img` resolves.
   */
  const embed = async (pending: PendingAttachment) => {
    try {
      const path = await writeAttachment(pending);
      // `[` and `]` in a claimed filename would close the alt text early. The
      // path never needs escaping: Rust names the file itself, from a
      // timestamp and the sniffed extension.
      input.current?.insertText(`![${pending.name.replace(/[[\]()]/g, "")}](${path})`);
      setAttachError(null);
    } catch (e) {
      setAttachError(String(e));
    } finally {
      // The strip the composer draws from this is not drawn here — the
      // picture goes straight into the text — so the preview URL is dead the
      // moment the write lands.
      releaseAttachment(pending);
    }
  };

  /** Pasted pictures, which arrive as `File`s the clipboard owns. One at a
   *  time, so two screenshots land in the order they were pasted. */
  const attach = async (files: File[]) => {
    for (const f of files) await embed(pendingFromFile(f));
  };

  /** …and dropped ones, which arrive as paths (`useFileDrop`). A drop of
   *  something else says so rather than being ignored, exactly as the
   *  composer does: silence reads as a broken drop target. */
  const attachPaths = (paths: string[]) => {
    if (!editing) return;
    const pictures = imagePaths(paths);
    if (!pictures.length) {
      if (paths.length) setAttachError("Only images can go in a task body — use @ for a course file.");
      return;
    }
    void (async () => {
      for (const p of pictures) await embed(pendingFromPath(p));
    })();
  };

  const dropping = useFileDrop(wrapRef, attachPaths);
  const body = draft.trim();

  return (
    // No `relative`: the `@` menu was the only thing positioned against this
    // wrapper, and it is `fixed` at the caret now — it portals out of here,
    // and stays written here because this is the editor it belongs to.
    <div ref={wrapRef} className="mt-6">
      <MentionMenu {...mentions.menu} />

      {attachError && (
        <div className="px-2 pb-1 text-[11px] text-destructive">{attachError}</div>
      )}

      {editing ? (
        <div
          className={cn(
            "rounded-lg border border-brand/40 bg-card px-2 py-1.5 transition-colors",
            dropping && "border-brand ring-[3px] ring-brand/25",
          )}
        >
          <MentionInput
            ref={input}
            autoFocus
            label="Description"
            initialText={draft}
            placeholder="Write what this actually involves…"
            // Taller than the composer's ten lines — this is a page, not a
            // message box — but still bounded, so a long body scrolls inside
            // the editor and leaves the subtasks and the delete row below it
            // reachable. The `@` menu no longer cares how tall this gets: it
            // is measured from the caret, so it follows the text being typed
            // rather than the box holding it.
            className="max-h-[420px] min-h-24 leading-relaxed"
            onEdit={(next, caret) => {
              setDraft(next);
              mentions.track(next, caret);
            }}
            onFiles={(files) => void attach(files)}
            onBlur={leave}
            onKeyDown={(e) => {
              // The menu's keys first, and only its own — Enter picks a file
              // while the list is open and breaks a line the rest of the time,
              // which is the composer's bargain minus the send.
              mentions.keyDown(e);
              if (e.defaultPrevented) return;
              // ⌘↵ saves without leaving the field — the composer's gesture,
              // and the only way to save a body you are not finished with.
              if (e.key === "Enter" && (e.metaKey || e.ctrlKey)) {
                e.preventDefault();
                save();
              }
            }}
          />
        </div>
      ) : (
        <div
          tabIndex={0}
          onClick={(e) => {
            // A chip, a link or a picture in the rendered body is a control of
            // its own — `FileChip` stops the click, the others do not — so a
            // click that landed on one would otherwise open the file *and* the
            // editor.
            const hit = (e.target as HTMLElement).closest("a,button,img,[role=button]");
            if (hit && hit !== e.currentTarget) return;
            setEditing(true);
          }}
          onKeyDown={(e) => {
            if (e.key !== "Enter") return;
            e.preventDefault();
            setEditing(true);
          }}
          className={cn(
            "min-h-24 cursor-text rounded-lg border border-transparent px-2 py-1.5 outline-none transition-colors",
            "hover:border-border-subtle focus-visible:border-brand/40",
          )}
        >
          {/* An empty body still needs a way in, and the invitation is it —
              there is nothing else to click on a task nobody has described. */}
          {body ? (
            // The outer margins come off so the rendered text sits exactly
            // where the editor's text does — otherwise clicking in nudges the
            // whole body up by a paragraph gap.
            <CompactMd text={body} className="[&>*:first-child]:mt-0 [&>*:last-child]:mb-0" />
          ) : (
            <span className="text-[13px] text-muted-foreground/60">
              Write what this actually involves…
            </span>
          )}
        </div>
      )}
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
  /** `null` on an unfiled task, whose board is `boardOf`'s default four. */
  project: DbProject | null;
  /** Not named `children`: that prop is JSX's, and a component taking one by
   *  that name is a component whose contents anyone can accidentally replace. */
  subtasks: DbProjectTask[];
  onToggle: (child: DbProjectTask, columnId: string) => void;
  onAdd: (title: string) => void;
}) {
  const doneColumn = boardOf(project).find((c) => c.kind === "done") ?? null;
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
