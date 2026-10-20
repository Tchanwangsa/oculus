import type { ComponentProps } from "react";
import type { DbProject, ProjectColumn } from "@/lib/planning/projects";
import { TasksBoard } from "../board/TasksBoard";
import { InlineAdd } from "../fields/InlineAdd";
import { TasksTable } from "../lists/TasksTable";
import { promotionTarget } from "./taskTree";

/** The same table and cards in Tasks and within a project. */
export function TasksView({ columns, project, onCreate, ...table }: ComponentProps<typeof TasksTable> & {
  columns: readonly ProjectColumn[];
  project?: DbProject;
  onCreate?: (input: { title: string; columnId: string }) => void;
}) {
  if (table.display.view === "board") {
    if (table.tasks.length === 0 && !onCreate) return <div className="flex h-full items-start justify-center px-6 py-16"><p className="max-w-sm text-center text-xs text-muted-foreground">{table.empty}</p></div>;
    return <TasksBoard tasks={table.tasks} allTasks={table.allTasks} columns={columns} project={project}
      projectById={table.projectById} tagById={table.tagById} properties={table.display.properties}
      onMove={table.actions.onMove} onCreate={onCreate} />;
  }
  const column = project ? promotionTarget(project) ?? project.columns[0] : null;
  return <TasksTable {...table} statusColumns={project?.columns} footer={onCreate && column ? (
    <div className="border-t border-border-subtle px-5 py-2"><InlineAdd label="New task" placeholder="Task title" onAdd={(title) => onCreate({ title, columnId: column.id })} /></div>
  ) : undefined} />;
}
