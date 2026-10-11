import { cn } from "@/lib/utils";
import type { DbProject, DbProjectTask } from "@/lib/planning/projects";
import { TaskGlyph } from "../tasks/TaskMarks";
import { columnOf } from "../tasks/taskTree";

/** Undated tasks are listed rather than hidden, like the calendar's "Due"
 *  strip. */
export function UnscheduledRail({
  project,
  tasks,
}: {
  project: DbProject;
  tasks: DbProjectTask[];
}) {
  return (
    <div className="shrink-0 border-t border-border-subtle px-5 py-2">
      <div className="flex items-baseline gap-2">
        <span className="text-[11px] font-medium text-muted-foreground">Unscheduled</span>
        <span className="text-[11px] tabular-nums text-muted-foreground/60">{tasks.length}</span>
      </div>
      <div className="mt-1.5 flex max-h-[68px] flex-wrap gap-1.5 overflow-y-auto">
        {tasks.map((t) => (
          <span
            key={t.id}
            title={t.title}
            className="inline-flex max-w-52 items-center gap-1.5 rounded-full border border-border-subtle bg-surface px-2 py-0.5 text-[11px]"
          >
            <TaskGlyph kind={columnOf(project, t.column_id)?.kind ?? null} size={10} />
            <span
              className={cn(
                "truncate",
                t.done_at ? "text-muted-foreground line-through" : "text-foreground",
              )}
            >
              {t.title}
            </span>
          </span>
        ))}
      </div>
    </div>
  );
}
