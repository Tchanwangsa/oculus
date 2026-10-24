import { useState } from "react";
import { Link } from "react-router-dom";
import { defineComponent } from "@openuidev/react-lang";
import { z } from "zod/v4";
import { useTaskList } from "@/hooks/data/useTaskList";
import { StatusPill } from "@/components/projects/board/StatusPill";
import { taskHref } from "@/components/projects/nav/taskHref";
import {
  boardOf,
  moveTask,
  type DbProject,
  type DbProjectTask,
} from "@/lib/planning/projects";
import { Frame } from "./Frame";
import { fmtRelativeDay, sqliteUtcToMs } from "@/lib/format/format";

function TaskRow({
  task,
  project,
}: {
  task: DbProjectTask;
  project: DbProject | null;
}) {
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const move = async (column: string) => {
    if (busy) return;
    setBusy(true);
    setError(null);
    try {
      await moveTask(task.id, column, null, null);
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  };
  const columns = boardOf(project);
  const toggle = () => {
    const target = columns.find((c) =>
      task.done_at ? c.kind !== "done" : c.kind === "done",
    );
    if (target) void move(target.id);
  };
  return (
    <div className="py-2">
      <div className="flex flex-wrap items-center gap-2">
        <input
          type="checkbox"
          aria-label={`Complete ${task.title}`}
          checked={!!task.done_at}
          disabled={busy || !columns.some((c) => c.kind === "done")}
          onChange={toggle}
          className="accent-primary"
        />
        <Link
          to={taskHref(task.project_id, task)}
          className="min-w-0 flex-1 text-sm hover:text-brand"
        >
          {task.title}
        </Link>
        <StatusPill
          project={project}
          columnId={task.column_id}
          onPick={(id) => void move(id)}
        />
        {task.due_at && (
          <span className="text-xs text-muted-foreground">
            {fmtRelativeDay(sqliteUtcToMs(task.due_at) ?? Date.now())}
          </span>
        )}
      </div>
      {error && (
        <p role="alert" className="mt-1 text-xs text-destructive">
          {error}
        </p>
      )}
    </div>
  );
}

export function LiveTask({ id }: { id: number }) {
  const data = useTaskList("all");
  const task = data.tasks.find((t) => t.id === id);
  return (
    <Frame title="Task">
      {task ? (
        <TaskRow
          task={task}
          project={data.projectById.get(task.project_id ?? -1) ?? null}
        />
      ) : (
        <p className="text-sm text-muted-foreground">
          {data.loaded ? "This task is no longer available." : "Loading task…"}
        </p>
      )}
    </Frame>
  );
}

export const TaskList = defineComponent({
  name: "TaskList",
  description:
    "Live tasks in an existing project; read the project id from the CLI.",
  props: z.object({ projectId: z.number().int().positive() }),
  component: function TaskListView({ props }) {
    const data = useTaskList(props.projectId);
    const project = data.projectById.get(props.projectId) ?? null;
    return (
      <Frame title={project?.name ?? "Tasks"}>
        <div className="divide-y divide-border-subtle">
          {data.tasks.map((task) => (
            <TaskRow key={task.id} task={task} project={project} />
          ))}
        </div>
        {!data.tasks.length && (
          <p className="text-sm text-muted-foreground">
            {data.loaded ? "No tasks in this project." : "Loading tasks…"}
          </p>
        )}
      </Frame>
    );
  },
});
