import { useMemo } from "react";
import { Link } from "react-router-dom";
import { ROW, Section } from "@/components/home/Section";
import { sqliteUtcToMs } from "@/lib/format/format";
import type { DbProject, DbProjectTask } from "@/lib/planning/projects";
import { DueChip, TaskGlyph } from "../tasks/TaskMarks";
import { taskHref } from "../nav/taskHref";
import { columnOf, type TaskNode } from "../tasks/taskTree";
import { Heading } from "./overviewParts";

const MAX_UPCOMING = 5;

interface Upcoming {
  task: DbProjectTask;
  /** Set on a subtask, for context. */
  parentTitle: string | null;
  due: number;
}

/** Unfinished, dated tasks and subtasks, soonest (so overdue) first. */
function upcomingTasks(nodes: TaskNode[]): Upcoming[] {
  const out: Upcoming[] = [];
  const take = (task: DbProjectTask, parentTitle: string | null) => {
    if (task.done_at != null) return;
    const due = sqliteUtcToMs(task.due_at);
    if (due == null) return;
    out.push({ task, parentTitle, due });
  };
  for (const node of nodes) {
    take(node.task, null);
    for (const child of node.children) take(child, node.task.title);
  }
  return out.sort((a, b) => a.due - b.due).slice(0, MAX_UPCOMING);
}

/** Two empty states (no tasks / no dates), neither in a bordered box. */
export function UpcomingTasks({ project, nodes }: { project: DbProject; nodes: TaskNode[] }) {
  const rows = useMemo(() => upcomingTasks(nodes), [nodes]);
  const anyTasks = nodes.length > 0;

  if (rows.length === 0) {
    return (
      <section>
        <Heading>Upcoming tasks</Heading>
        <p className="px-0.5 text-xs text-muted-foreground">
          {anyTasks
            ? "Nothing here has a date on it yet. A task needs a due date before it can be next — and before the calendar can draw it."
            : "No tasks yet. Break this down on the Tasks tab and what is next will show up here."}
        </p>
      </section>
    );
  }

  return (
    <Section title="Upcoming tasks">
      {rows.map(({ task, parentTitle }) => (
        <Link key={task.id} to={taskHref(project.id, task)} className={ROW}>
          <TaskGlyph kind={columnOf(project, task.column_id)?.kind ?? null} />
          <span className="min-w-0 flex-1">
            <span className="block truncate text-[12px] text-foreground">{task.title}</span>
            {parentTitle && (
              <span className="block truncate text-[11px] text-muted-foreground">
                {parentTitle}
              </span>
            )}
          </span>
          <DueChip dueAt={task.due_at} />
        </Link>
      ))}
    </Section>
  );
}
