import { AgentMark } from "@/components/projects/tasks/TaskMarks";
import { fmtClock, sqliteUtcToMs } from "@/lib/format/format";
import type { DbProjectTask } from "@/lib/planning/projects";
import { ageLabel } from "./taskFormat";

/** The footer: when the task was created and last changed, and whether the
 *  chat agent wrote it. The columns are SQLite's naive UTC, so they go
 *  through `sqliteUtcToMs`, never `new Date(string)`. */
export function TaskMeta({ task }: { task: DbProjectTask }) {
  const created = sqliteUtcToMs(task.created_at);
  const updated = sqliteUtcToMs(task.updated_at);
  return (
    <footer className="mt-10 flex flex-wrap items-center gap-x-1.5 border-t border-border-subtle pt-4 text-[11px] text-muted-foreground tabular-nums">
      {created != null && (
        <span title={fmtClock(created, true)}>Created {ageLabel(created)}</span>
      )}
      {created != null && updated != null && <span aria-hidden>·</span>}
      {updated != null && (
        <span title={fmtClock(updated, true)}>Updated {ageLabel(updated)}</span>
      )}
      <AgentMark source={task.source} className="ml-1" />
    </footer>
  );
}
