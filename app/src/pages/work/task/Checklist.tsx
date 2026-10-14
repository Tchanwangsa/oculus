import { Link } from "react-router-dom";
import { Check } from "@phosphor-icons/react";
import { InlineAdd } from "@/components/projects/fields/InlineAdd";
import { AgentMark, DueChip } from "@/components/projects/tasks/TaskMarks";
import { taskHref } from "@/components/projects/nav/taskHref";
import { promotionTarget } from "@/components/projects/tasks/taskTree";
import { boardOf, type DbProject, type DbProjectTask } from "@/lib/planning/projects";
import { cn } from "@/lib/utils";

/**
 * The task's subtasks as a checklist. A tick is a `moveTask` between the done
 * column and the first work column, so `done_at` and the column never
 * disagree; a board with no such column leaves the box disabled and says why.
 */
export function Checklist({
  project,
  items,
  onToggle,
  onAdd,
}: {
  /** `null` on an unfiled task, whose board is `boardOf`'s default four. */
  project: DbProject | null;
  /** The subtasks (not `children`, which is JSX's). */
  items: DbProjectTask[];
  onToggle: (item: DbProjectTask, columnId: string) => void;
  onAdd: (title: string) => void;
}) {
  const doneColumn = boardOf(project).find((c) => c.kind === "done") ?? null;
  const activeColumn = promotionTarget(project);

  /** Where a tick would send this item, or null if nowhere. */
  const destination = (item: DbProjectTask) =>
    (item.done_at != null ? activeColumn : doneColumn)?.id ?? null;

  return (
    <section aria-label="Checklist" className="mt-6">
      {items.length > 0 && (
        <ul className="flex flex-col">
          {items.map((item) => {
            const target = destination(item);
            const done = item.done_at != null;
            const action = done ? "Mark as not done" : "Mark as done";
            return (
              <li
                key={item.id}
                className="-mx-1.5 flex items-center gap-2.5 rounded-lg px-1.5 py-1.5 transition-colors hover:bg-surface"
              >
                <button
                  type="button"
                  role="checkbox"
                  aria-checked={done}
                  aria-label={action}
                  title={target == null ? "This board has no column to move it to" : action}
                  disabled={target == null}
                  onClick={() => target && onToggle(item, target)}
                  className={cn(
                    "flex size-4 shrink-0 items-center justify-center rounded-[5px] border outline-none transition-colors focus-visible:ring-[3px] focus-visible:ring-ring/50",
                    done
                      ? "border-brand bg-brand text-brand-foreground"
                      : "border-muted-foreground/40 hover:border-brand",
                    target == null ? "cursor-not-allowed opacity-30" : "cursor-pointer",
                  )}
                >
                  {done && <Check size={10} weight="bold" />}
                </button>

                <Link
                  to={taskHref(item.project_id, item)}
                  className={cn(
                    "min-w-0 flex-1 truncate text-sm hover:underline",
                    done ? "text-muted-foreground line-through" : "text-foreground",
                  )}
                >
                  {item.title}
                </Link>

                <AgentMark source={item.source} />
                <DueChip dueAt={item.due_at} />
              </li>
            );
          })}
        </ul>
      )}

      <InlineAdd
        label="Add checklist item"
        placeholder="One piece of this"
        // `createTask` files a subtask in its parent's column.
        onAdd={onAdd}
        className="-mx-1.5 gap-2.5 px-1.5 text-sm"
        inputClassName="text-sm"
      />
    </section>
  );
}
