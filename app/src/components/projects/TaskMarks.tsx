import { CaretRight, CheckCircle, Circle, CircleDashed, Sparkle } from "@phosphor-icons/react";
import { cn } from "@/lib/utils";
import { Progress } from "@/components/ui/progress";
import { displayCode, fmtClock, sqliteUtcToMs } from "@/lib/format";
import { SubjectIcon } from "@/components/subjects/SubjectIcon";
import type { ColumnKind, ProjectTaskCounts } from "@/lib/projects";
import type { SubtaskProgress } from "./taskTree";

/** Reads the column's *kind*, never its user-editable name. */
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

/** Marks rows the chat agent wrote. */
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

/** A due date, red once past; renders nothing when unset. No `tabular-nums`:
 *  month names are different widths, so dates can't align anyway. */
export function DueChip({ dueAt, className }: { dueAt: string | null; className?: string }) {
  const ms = sqliteUtcToMs(dueAt);
  if (ms == null) return null;
  const overdue = ms < Date.now();
  return (
    <span
      className={cn(
        "shrink-0 text-[11px]",
        overdue ? "text-destructive" : "text-muted-foreground",
        className,
      )}
    >
      {fmtClock(ms, true)}
    </span>
  );
}

/** `3/12` and a thin bar. Callers handle `total === 0` themselves — it is
 *  never a 0% bar. */
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

/** A project's tasks (subtasks included, via `getTaskCounts`) on a list row. */
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

/** An unfiled task has no subject; a subject-less project is Personal. */
export function TaskSubject({ code, unfiled = false }: { code: string | null; unfiled?: boolean }) {
  if (unfiled) return <span className="text-[11px] text-muted-foreground/50">—</span>;
  if (!code) return <span className="text-[11px] text-muted-foreground/60">Personal</span>;
  return (
    <span className="inline-flex min-w-0 items-center gap-1.5 rounded-full border border-border-subtle bg-surface px-2 py-0.5 text-[11px] text-foreground">
      <SubjectIcon code={code} size={11} />
      <span className="truncate">{displayCode(code)}</span>
    </span>
  );
}

/** Reserve the caret's space even when the task has no children. */
export function SubtaskToggle({ expandable, expanded, onToggle }: {
  expandable: boolean;
  expanded: boolean;
  onToggle: () => void;
}) {
  return expandable ? (
    <button
      type="button"
      aria-label={expanded ? "Collapse subtasks" : "Expand subtasks"}
      aria-expanded={expanded}
      onClick={onToggle}
      className="shrink-0 cursor-pointer p-0.5 text-muted-foreground/50 transition-colors hover:text-foreground"
    >
      <CaretRight size={9} className={cn("transition-transform", expanded && "rotate-90")} />
    </button>
  ) : <span aria-hidden className="w-[13px] shrink-0" />;
}
