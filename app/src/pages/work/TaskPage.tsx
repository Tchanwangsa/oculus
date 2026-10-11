import { useState } from "react";
import { Link, Navigate, useNavigate } from "react-router-dom";
import { TrashSimple } from "@phosphor-icons/react";
import { Button } from "@/components/ui/button";
import { ConfirmDialog } from "@/components/ui/ConfirmDialog";
import { StatusPill } from "@/components/projects/board/StatusPill";
import { AgentMark } from "@/components/projects/tasks/TaskMarks";
import { DateTimeField } from "@/components/projects/fields/DateTimeField";
import { DraftField } from "@/components/projects/fields/DraftField";
import { projectHref } from "@/components/projects/nav/projectHref";
import { ProjectCrumbs } from "@/components/projects/page/ProjectCrumbs";
import { ProjectPicker } from "@/components/projects/nav/ProjectPicker";
import { PaneHeaderRow, PaneTrail, useInSidePanel } from "@/components/tabs/PaneHeader";
import { navigateActive } from "@/lib/shell/tabRouters";
import { taskHref } from "@/components/projects/nav/taskHref";
import { fmtAgo, fmtClock, sqliteUtcToMs } from "@/lib/format/format";
import { cn } from "@/lib/utils";
import { PropertyRow } from "./task/PropertyRow";
import { Subtasks } from "./task/Subtasks";
import { TaskBody } from "./task/TaskBody";
import { TaskTitle } from "./task/TaskTitle";
import { useTaskMutations } from "./task/useTaskMutations";
import { useTaskPageData } from "./task/useTaskPageData";

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
export default function TaskPage() {
  const navigate = useNavigate();
  const inSide = useInSidePanel();
  const [confirmDelete, setConfirmDelete] = useState(false);

  const { filed, id, tid, project, projects, ready, task, parent, children, nodes } =
    useTaskPageData();
  const { patch, move, refile, createTask, deleteTask } = useTaskMutations(task, nodes);

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
          <PropertyRow label="Status">
            <StatusPill
              project={project}
              columnId={task.column_id}
              onPick={(columnId) => move(task.id, columnId)}
            />
          </PropertyRow>

          <PropertyRow label="Project">
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
          </PropertyRow>

          <PropertyRow label="Due">
            <DateTimeField
              value={task.due_at}
              defaultTime="end"
              onCommit={(dueAt) => patch({ dueAt })}
            />
          </PropertyRow>

          <PropertyRow label="Starts">
            <DateTimeField
              value={task.starts_at}
              defaultTime="start"
              onCommit={(startsAt) => patch({ startsAt })}
            />
          </PropertyRow>

          <PropertyRow label="Estimate">
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
          </PropertyRow>

          <PropertyRow label="Parent">
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
          </PropertyRow>

          <PropertyRow label="Added">
            <span className="text-xs text-muted-foreground">
              {/* Not `fmtDate(created_at)`: the column is SQLite's naive UTC. */}
              {createdMs ? fmtClock(createdMs, true) : "—"}
            </span>
            {createdMs != null && (
              <span className="text-[11px] text-muted-foreground/60">{fmtAgo(createdMs)}</span>
            )}
            <AgentMark source={task.source} />
          </PropertyRow>
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
