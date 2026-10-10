import { useState } from "react";
import { Check } from "@phosphor-icons/react";
import { cn } from "@/lib/utils";
import { Popover, PopoverContent, PopoverTrigger } from "@/components/ui/popover";
import { boardOf, type DbProject, type ProjectColumn } from "@/lib/planning/projects";

/**
 * A column pill's tint, by kind plus rank among the active columns (Todo and
 * In progress are both `active`): backlog outline → queued grey → in flight
 * brand → done success. An unknown column is painted destructive.
 */
function columnPillClass(columns: ProjectColumn[], columnId: string): string {
  const column = columns.find((c) => c.id === columnId) ?? null;
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
 * A task's column as an editable pill. Callers write through `moveTask`, never
 * `updateTask`: only `moveTask` keeps `done_at` in step with the column.
 */
export function StatusPill({
  project,
  columnId,
  onPick,
  className,
}: {
  /** `null` for an unfiled task (default board). */
  project: DbProject | null;
  columnId: string;
  onPick: (columnId: string) => void;
  className?: string;
}) {
  const [open, setOpen] = useState(false);
  const columns = boardOf(project);
  const column = columns.find((c) => c.id === columnId) ?? null;

  return (
    <Popover open={open} onOpenChange={setOpen}>
      <PopoverTrigger asChild>
        <button
          type="button"
          aria-label="Change status"
          className={cn(
            "inline-flex max-w-full cursor-pointer items-center rounded-full border px-2 py-0.5 text-[11px] font-medium transition-opacity hover:opacity-80",
            columnPillClass(columns, columnId),
            className,
          )}
        >
          <span className="truncate">{column?.name ?? columnId}</span>
        </button>
      </PopoverTrigger>
      <PopoverContent align="start" className="w-44 p-1">
        {columns.map((c) => (
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
                columnPillClass(columns, c.id),
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
