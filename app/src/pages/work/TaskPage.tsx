import { useState } from "react";
import { Link, Navigate, useNavigate } from "react-router-dom";
import { ConfirmDialog } from "@/components/ui/ConfirmDialog";
import { projectHref } from "@/components/projects/nav/projectHref";
import { taskHref } from "@/components/projects/nav/taskHref";
import { TaskFiles } from "@/components/projects/task/TaskFiles";
import { TaskHeader } from "@/components/projects/task/TaskHeader";
import { TaskMeta } from "@/components/projects/task/TaskMeta";
import { TaskProperties } from "@/components/projects/task/TaskProperties";
import { Checklist } from "./task/Checklist";
import { TaskBody } from "./task/TaskBody";
import { TaskTitle } from "./task/TaskTitle";
import { useTaskMutations } from "./task/useTaskMutations";
import { useTaskPageData } from "./task/useTaskPageData";

/**
 * One task as a page of its own: title, a row of property pills, an editable
 * description, its subtasks as a checklist and its related files. It reads
 * the whole project, because a status only means something against the
 * project's own columns and `moveTask` needs a column id from it.
 *
 * Two routes, one page: a filed task is `/projects/:projectId/tasks/:taskId`,
 * an unfiled one (no project at all) is `/tasks/:taskId`. `project` is
 * therefore nullable and the board is read through `boardOf`, which gives the
 * default four columns when there is none. The store holds one open project's
 * tasks, so the unfiled half reads its rows through `useTaskList` instead.
 *
 * The Project pill is a picker over `refileTask`, and a refile re-navigates to
 * the task's new href, since that href is built from the project.
 */
export default function TaskPage() {
  const navigate = useNavigate();
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

  // Subtasks are one level deep, so a subtask has no checklist.
  const isSubtask = task.parent_id != null;

  return (
    <div className="flex h-full min-h-0 flex-col">
      <TaskHeader task={task} project={project} parent={parent} onDelete={() => setConfirmDelete(true)} />

      <div className="page-scroll min-h-0 flex-1">
        <div className="mx-auto max-w-3xl px-6 pt-8 pb-16">
          <TaskTitle task={task} onRename={(title) => patch({ title })} />

          {parent && (
            <p className="mt-1.5 flex min-w-0 items-center gap-1.5 text-xs text-muted-foreground">
              <span className="shrink-0">Subtask of</span>
              <Link
                to={taskHref(task.project_id, parent)}
                className="min-w-0 truncate text-foreground hover:underline"
              >
                {parent.title}
              </Link>
            </p>
          )}

          <TaskProperties
            task={task}
            project={project}
            projects={projects}
            checklist={children}
            onMove={(columnId) => move(task.id, columnId)}
            onRefile={refile}
            onPatch={patch}
          />

          {/* Keyed by task so another task starts fresh. Siblings never share a
              key: a shared one makes React drop an old sibling from the tree
              without unmounting it, leaving its editor on the page. */}
          <TaskBody
            key={`body-${task.id}`}
            task={task}
            project={project}
            onSave={(body) => patch({ body })}
          />

          {!isSubtask && (
            <Checklist
              project={project}
              items={children}
              onToggle={(item, columnId) => move(item.id, columnId)}
              onAdd={(title) =>
                // In the parent's project, even when that is none.
                createTask({ projectId: task.project_id, parentId: task.id, title }).catch((e) =>
                  console.error("create subtask failed", e),
                )
              }
            />
          )}

          <TaskFiles key={`files-${task.id}`} taskId={task.id} />

          <TaskMeta task={task} />
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
}
