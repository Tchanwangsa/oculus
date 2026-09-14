import { CheckCircle, Circle, CircleDashed, Sparkle } from "@phosphor-icons/react";
import { cn } from "@/lib/utils";
import { Progress } from "@/components/ui/progress";
import { fmtClock, sqliteUtcToMs } from "@/lib/format";
import type { ColumnKind, ProjectTaskCounts } from "@/lib/projects";
import type { SubtaskProgress } from "./taskTree";

/**
 * What a task is, at a glance: the glyph reads the *kind* of the column it
 * sits in rather than the column's name, which is the user's to change — the
 * same rule `moveTask` applies when it decides whether the task is done.
 */
export function TaskGlyph({
  kind,
  size = 13,
  className,
}: {
  kind: ColumnKind | null;
  size?: number;
  className?: string;
}) {
  if (kind === "done") {
    return (
      <CheckCircle
        size={size}
        weight="fill"
        className={cn("shrink-0 text-success", className)}
      />
    );
  }
  if (kind === "backlog") {
    return (
      <CircleDashed size={size} className={cn("shrink-0 text-muted-foreground/60", className)} />
    );
  }
  return <Circle size={size} className={cn("shrink-0 text-muted-foreground/70", className)} />;
}

/** Quiet marker for anything the chat agent wrote, so what you planned and
 *  what it planned for you stay tellable apart. */
export function AgentMark({ source, className }: { source: string; className?: string }) {
  if (source !== "agent") return null;
  return (
    <Sparkle
      size={11}
      weight="fill"
      aria-label="Added by the chat agent"
      className={cn("shrink-0 text-brand/60", className)}
    />
  );
}

/** A due date, reddened once it is behind you. Unset dates render nothing so
 *  the caller can decide what an empty cell looks like. */
export function DueChip({ dueAt, className }: { dueAt: string | null; className?: string }) {
  const ms = sqliteUtcToMs(dueAt);
  if (ms == null) return null;
  const overdue = ms < Date.now();
  return (
    <span
      className={cn(
        "shrink-0 text-[11px] tabular-nums",
        overdue ? "text-destructive" : "text-muted-foreground",
        className,
      )}
    >
      {fmtClock(ms, true)}
    </span>
  );
}

/**
 * `3/12` and a thin bar — the shared half of every progress readout here.
 *
 * A task row and a project row want the same meter at different weights: the
 * table has a column of its own to fill and can afford the percentage, while a
 * project row is already carrying a name, a brief and a due date and the
 * fraction says enough. So the percentage is a flag rather than a second
 * component. Callers handle `total === 0` themselves — what "nothing yet"
 * should say differs by row, and it is never a 0% bar.
 */
export function ProgressMeter({
  done,
  total,
  showPercent = true,
  barClassName = "w-12",
  className,
}: {
  done: number;
  total: number;
  showPercent?: boolean;
  barClassName?: string;
  className?: string;
}) {
  const pct = total ? Math.round((done / total) * 100) : 0;
  return (
    <span className={cn("flex items-center gap-2", className)}>
      <span className="text-[11px] tabular-nums text-muted-foreground">
        {done}/{total}
      </span>
      <Progress
        value={pct}
        className={cn("h-1 shrink-0", barClassName)}
        indicatorClassName={pct === 100 ? "bg-success" : undefined}
      />
      {showPercent && (
        <span className="text-[11px] tabular-nums text-muted-foreground/70">{pct}%</span>
      )}
    </span>
  );
}

/** A task's subtasks. Nothing at all when it has none — a 0% bar reads as
 *  "none done" rather than "none exist", and this sits in a table column where
 *  a dash is the established empty. */
export function SubtaskProgressBar({
  progress,
  className,
}: {
  progress: SubtaskProgress;
  className?: string;
}) {
  if (progress.total === 0) {
    return <span className={cn("text-[11px] text-muted-foreground/50", className)}>—</span>;
  }
  return (
    <ProgressMeter done={progress.done} total={progress.total} className={className} />
  );
}

/**
 * A whole project's tasks, on a list row.
 *
 * An empty project says so in words rather than drawing `0/0` behind an empty
 * bar: a bar at zero is what a stalled project looks like, and one you have
 * not broken down yet is not stalled. Counts come from `getTaskCounts`, which
 * includes subtasks — a project taken apart properly should not read as barely
 * started.
 */
export function ProjectProgress({
  counts,
  className,
}: {
  counts: ProjectTaskCounts | undefined;
  className?: string;
}) {
  if (!counts || counts.total === 0) {
    return (
      <span className={cn("shrink-0 text-[11px] text-muted-foreground/60", className)}>
        Nothing planned yet
      </span>
    );
  }
  return (
    <ProgressMeter
      done={counts.done}
      total={counts.total}
      showPercent={false}
      barClassName="w-16"
      className={cn("shrink-0", className)}
    />
  );
}
