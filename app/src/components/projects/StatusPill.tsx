import { useState } from "react";
import { Check } from "@phosphor-icons/react";
import { cn } from "@/lib/utils";
import { Popover, PopoverContent, PopoverTrigger } from "@/components/ui/popover";
import type { DbProject, ProjectColumn } from "@/lib/projects";

/**
 * The tint a column's pill wears.
 *
 * Kind alone is not enough: Todo and In progress are both `kind: "active"`,
 * and the pill *is* the whole Status cell in the table, so two different
 * states read identically. So the ramp is kind plus position — how far along
 * the project's own board the column sits — and it runs from no fill at all to
 * the brand accent:
 *
 *   backlog → outline only · queued → grey fill · in flight → brand · done → success
 *
 * The last active column takes the brand because `brand` is this palette's
 * accent for work in flight (the root `CLAUDE.md`), and the columns before it
 * step down towards the neutral end rather than taking hues of their own —
 * a board is not a legend, and three saturated pills in a Status column would
 * read as three warnings.
 */
export function columnPillClass(columns: ProjectColumn[], columnId: string): string {
  const column = columns.find((c) => c.id === columnId) ?? null;
  // A column the project no longer has: the task still names it, and saying so
  // is more use than drawing it as though it were filed correctly.
  if (!column) return "border-destructive/20 bg-destructive/10 text-destructive";
  if (column.kind === "done") return "border-success/20 bg-success/15 text-success";
  if (column.kind === "backlog") return "border-border bg-transparent text-muted-foreground";

  const active = columns.filter((c) => c.kind === "active");
  const rank = active.findIndex((c) => c.id === column.id);
  const last = active.length - 1;
  if (rank === last) return "border-brand/25 bg-brand-muted text-brand";
  if (rank === 0) return "border-border bg-secondary text-foreground";
  return "border-brand/20 bg-brand-muted/50 text-brand/80";
}

/**
 * The board column a task sits in, as a pill you can change in place.
 *
 * It calls `moveTask` — never `updateTask`, which cannot write `column_id` at
 * all: the column and `done_at` are one fact, and only `moveTask` reads the
 * project's board to learn whether the destination is a `kind: "done"` one.
 */
export function StatusPill({
  project,
  columnId,
  onPick,
  className,
}: {
  project: DbProject;
  columnId: string;
  /** Given a column id from the project's own board — never an invented one. */
  onPick: (columnId: string) => void;
  className?: string;
}) {
  const [open, setOpen] = useState(false);
  const column = project.columns.find((c) => c.id === columnId) ?? null;

  return (
    <Popover open={open} onOpenChange={setOpen}>
      <PopoverTrigger asChild>
        <button
          type="button"
          aria-label="Change status"
          className={cn(
            "inline-flex max-w-full cursor-pointer items-center rounded-full border px-2 py-0.5 text-[11px] font-medium transition-opacity hover:opacity-80",
            columnPillClass(project.columns, columnId),
            className,
          )}
        >
          <span className="truncate">{column?.name ?? columnId}</span>
        </button>
      </PopoverTrigger>
      <PopoverContent align="start" className="w-44 p-1">
        {project.columns.map((c) => (
          <button
            key={c.id}
            type="button"
            onClick={() => {
              setOpen(false);
              if (c.id !== columnId) onPick(c.id);
            }}
            className="flex w-full cursor-pointer items-center gap-2 rounded-md px-2 py-1.5 text-left text-xs text-foreground transition-colors hover:bg-accent"
          >
            <span
              aria-hidden
              className={cn(
                "size-2 shrink-0 rounded-full border",
                columnPillClass(project.columns, c.id),
              )}
            />
            <span className="min-w-0 flex-1 truncate">{c.name}</span>
            {c.id === columnId && <Check size={12} className="shrink-0 text-brand" />}
          </button>
        ))}
      </PopoverContent>
    </Popover>
  );
}
