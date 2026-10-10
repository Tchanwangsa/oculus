import { Link } from "react-router-dom";
import { InlineAdd } from "@/components/projects/fields/InlineAdd";
import { AgentMark, DueChip, TaskGlyph } from "@/components/projects/tasks/TaskMarks";
import { taskHref } from "@/components/projects/nav/taskHref";
import { columnOf, promotionTarget } from "@/components/projects/tasks/taskTree";
import { boardOf, type DbProject, type DbProjectTask } from "@/lib/planning/projects";
import { cn } from "@/lib/utils";
import { ListCard } from "@/components/ui/layout/PageParts";

/**
 * The task's children. A tick is a `moveTask` between the done column and the
 * first work column, so `done_at` and the column never disagree. A board with
 * no such column leaves the control disabled and says why.
 */
export function Subtasks({
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
